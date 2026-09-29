use crate::transport::node_transport::proto::{DownloadRequest, FileChunk};
use crate::transport::node_transport_spec::{CapabilitiesRequest, ConnectServerRequest, CreateServerRequest, CustomNodeTransportable, DeleteServerRequest, FileDownloadRequest, FileUploadRequest, FilterRequest, IntegrationKeyRequest, MigrateRequest, NodeTransportable, NodeTransportableMut, Ping, ServerDataRequest, ServerStateRequest, ServernameRequest, SetServerRequest, StartServerRequest, StopServerRequest, StreamTransportable};
use crate::transport::node_transport_spec::RemoteFile;
use crate::{ApiCalls as ToplevelApiCalls, AuthTcpMessage, IncomingMessage, List, NodeWithConn, StreamResult, UserClient};
use crate::{
    AppState, MessagePayload, MessagePayloadWithMetadata, MetadataTypes, SimpleMessage,
    ServerStatus
};
use crate::{
    CHANNEL_BUFFER_SIZE, ConsoleData,
    transport::node_transport::proto::{ServerMessage, node_manage_client::NodeManageClient},
};
use arc_swap::ArcSwap;
use dashmap::DashMap;
use futures_util::{Stream, StreamExt, stream};
use general_networked_filesystem::core::LsRequest;
use general_networked_filesystem::core::DirectoryResponse;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{Mutex, Notify, watch};
use tokio::{
    sync::{RwLock, broadcast, mpsc},
    time::timeout,
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{error::Error, sync::Arc};

use tonic::transport::Channel;
mod proto {
    tonic::include_proto!("main");
}
use proto::{
    filesystem_manage_client::FilesystemManageClient, general_client::GeneralClient,
    server_edit_client::ServerEditClient, server_manage_client::ServerManageClient,
};

#[derive(Clone)]
pub struct Clients {
    general_client: GeneralClient<Channel>,
    node_client: NodeManageClient<Channel>,
    server_manage_client: ServerManageClient<Channel>,
    server_edit_client: ServerEditClient<Channel>,
    filesystem_client: FilesystemManageClient<Channel>,
}


pub struct ConnectionHandler {
    //stream: Option<&'static TcpStream>,
    clients: Arc<Option<Clients>>,
    end_conn_task: Arc<Notify>,
    pub(crate) proxy_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    pub(crate) proxy_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    pub(crate) tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    pub(crate) rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
}


impl ConnectionHandler {
    pub fn new() -> Self {
        let (tx, rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (proxy_tx, proxy_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        ConnectionHandler {
            //stream: None,
            proxy_tx,
            proxy_rx,
            tx,
            rx,
            clients: Arc::new(None),
            end_conn_task: Arc::new(Notify::new()),
        }
    }
    pub fn end_connection(&self){
        self.end_conn_task.notify_waiters();
    }
    pub fn get_filesystem_stream(
        &self,
    ) -> (broadcast::Sender<Vec<u8>>, broadcast::Receiver<Vec<u8>>) {
        (self.proxy_tx.clone(), self.proxy_rx.resubscribe())
    }
}
impl Default for ConnectionHandler {
    fn default() -> Self {
        let (tx, rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (proxy_tx, proxy_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        ConnectionHandler {
            //stream: None,
            proxy_tx,
            proxy_rx,
            tx,
            rx,
            clients: Arc::new(None),
            end_conn_task: Arc::new(Notify::new()),
        }
    }
}
impl Clone for ConnectionHandler {
    fn clone(&self) -> Self {
        ConnectionHandler {
            //stream: None,
            clients: self.clients.clone(),
            proxy_tx: self.proxy_tx.clone(),
            proxy_rx: self.proxy_rx.resubscribe(),
            tx: self.tx.clone(),
            rx: self.rx.resubscribe(),
            end_conn_task: self.end_conn_task.clone()
        }
    }
}

// does the connection to the tcp server, wether initial or not, on success it will pass it off to the dedicated handler for the stream
pub async fn connect_to_server(
    arc_state: Arc<ArcSwap<AppState>>,
    url: String,
    _end_if_timeout: bool,
) -> Result<ConnectionHandler, Box<dyn Error + Send + Sync>> {
    println!("using this connect to server");
    let mut state = (*arc_state.load_full()).clone();

    let url = if url.starts_with("http://") || url.starts_with("https://") {
        url
    } else {
        format!("http://{url}")
    };

    let channel = Channel::from_shared(url.clone())?.connect().await?;

    let general_client = GeneralClient::new(channel.clone());
    let filesystem_client = FilesystemManageClient::new(channel.clone());
    let server_edit_client = ServerEditClient::new(channel.clone());
    let server_manage_client = ServerManageClient::new(channel.clone());
    let node_client = NodeManageClient::new(channel.clone());

    let mut connection_handler = ConnectionHandler::default();
    let mut clients = Arc::new(Some(Clients {
        general_client,
        node_client,
        server_manage_client,
        server_edit_client,
        filesystem_client,
    }));
    connection_handler.clients = clients.clone();
    tokio::spawn(
        async move {
            clients = Arc::new(None);
        }
    );

    arc_state.store(Arc::new(state));
  
    Ok(connection_handler)
}

// this is where it determines wether or not to try and create the container and deployment, as attempt_connection itself is used in various diffrent contexts (like it will constantly
// try to connect upon failing but it should not try to create the container and deployment every time it fails)
// I use anyhow here because it saves me having to try and downcast the error type
pub async fn try_initial_connection(
    _conn_attempts: u64,
    _conn_timeout: u64,
    _create_handler: bool,
    _state: &Arc<ArcSwap<AppState>>,
    _tcp_url: String,
) -> Result<(), anyhow::Error> {
    Ok(())
}

impl CustomNodeTransportable for ServernameRequest {
    type Output = String;

    async fn custom_node_transport(&self, state: &AppState, connection_handler: &ConnectionHandler) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        let server_name_request = proto::ServerNameRequest {
            r#type: "server_name".to_string(),
            message: "".to_string(),
            authcode: "0".to_string(),
        };
        match clients
            .node_client
            .clone()
            .name(server_name_request)
            .await
        {
            Ok(server_name) => {
                Ok(server_name.get_ref().message.clone())
            }
            Err(e) => {
                Ok("main".to_string())
            }
        }
    }
}

impl NodeTransportable for ServernameRequest {
    type Output = String;

    async fn node_transport(&self, state: &AppState) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        self.custom_node_transport(state, &connection_handler).await
    }
}

// NodeTransportable
impl NodeTransportable for DeleteServerRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        let request = proto::DeleteServerRequest {
            metadata: Some(self.metadata.clone().into()),
        };
        let _ = clients
            .server_edit_client
            .clone()
            .delete(request);

        Ok(())
    }
}

impl NodeTransportable for CreateServerRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        let request = proto::CreateServerRequest {
            metadata: Some(self.metadata.clone().into()),
        };
        let _ = clients
            .server_edit_client
            .clone()
            .create(request);

        Ok(())
    }
}

#[derive(Default, Debug)]
enum ConsoleEvent {
    #[default]
    None,
    Starting,
    Started
}

#[derive(Clone, Default)]
pub struct ConsoleInterface {
    active_event: Arc<watch::Sender<ConsoleEvent>>,
    close_task_event: Arc<Notify>,
    console_out: Arc<Mutex<Option<Pin<Box<dyn Stream<Item = ConsoleData> + Send + 'static>>>>>,
    console_in: Arc<Mutex<Option<mpsc::Sender<ServerMessage>>>>,
    proxy_out: Arc<RwLock<Vec<UnboundedSender<ConsoleData>>>>,
    proxy_in: Arc<Mutex<Option<broadcast::Sender<ConsoleData>>>>
}

impl ConsoleInterface {
    pub async fn spawn(&mut self, _state: Arc<ArcSwap<AppState>>) -> Result<(), Box<dyn Error + Send + Sync>>{
        loop {
            if matches!(*self.active_event.borrow(), ConsoleEvent::Starting){
                break;
            } else {
                let _ = self.active_event.subscribe().changed().await;
            }
        }
        let Some(ref mut console_out) = *self.console_out.lock().await else {
            return Err("no console out".into())
        };

        // self.active.store(true, Ordering::SeqCst);
        let (proxy_in_tx, mut proxy_in_rx) = broadcast::channel::<ConsoleData>(32);

        *self.proxy_in.lock().await = Some(proxy_in_tx);
        *self.proxy_out.write().await = Vec::new();

        let _ = self.active_event.send_replace(ConsoleEvent::Started);
        let close_task_event = self.close_task_event.clone();

        loop {
            tokio::select! {
                console_data_res = console_out.next() => {
                    if let Some(console_data) = console_data_res {
                        let proxy_out = self.proxy_out.read().await;
                        for tx in &*proxy_out {
                            let _ = tx.send(console_data.clone());
                        }
                    } else {
                        break;
                    }
                },
                Ok(console_data) = proxy_in_rx.recv(), if self.console_in.lock().await.is_some()  => {
                    let _ = self.console_in.lock().await.as_ref().unwrap().send(
                        ServerMessage { 
                            authcode: "0".into(), 
                            data: console_data.data, 
                            message: "console".into(), 
                            channel: "stdout".into(), 
                            servername: "unknown".into(),
                        }
                    ).await;
                }
                _ = close_task_event.notified() => {
                    break;
                }
            }
        }
        let _ = self.active_event.send(ConsoleEvent::None);
        Err("console disconnected".into())
    }
    pub async fn activate(&self){
        loop {
            if !matches!(*self.active_event.borrow(), ConsoleEvent::Started){
                let _ = self.active_event.subscribe().changed().await;
            } else {
                break;
            }
        }
        println!("calling active");
    }
    pub fn active(&self) -> bool {
        if matches!(*self.active_event.borrow(), ConsoleEvent::Started){
            true
        } else {
            false
        }
    }
    pub fn close(&self) {
        self.close_task_event.notify_waiters();
    }
    pub async fn stdin(&self) -> Option<broadcast::Sender<ConsoleData>> {
        self.proxy_in.lock().await.clone()
    }
    pub async fn stdout(&mut self) -> Option<UnboundedReceiver<ConsoleData>> {
        let (proxy_out_tx, proxy_out_rx) = mpsc::unbounded_channel();
        self.proxy_out.write().await.push(proxy_out_tx);
        if matches!(*self.active_event.borrow(), ConsoleEvent::Started) {
            Some(proxy_out_rx)
        } else {
            None
        }
    }
}

impl StreamTransportable for CreateServerRequest {
    type Output = ();
    async fn stream_transport(
        &mut self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let state = arc_state.load();
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };

        let request = proto::CreateServerRequest {
            metadata: Some(self.metadata.clone().into()),
        };

        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        println!("about to call create");
        match clients.server_edit_client.clone().create(request).await {
            Ok(response_stream) => {
                //drop(state);
                println!("before creating stream");
                let raw_stream = response_stream.into_inner();
                let stream = stream::unfold(raw_stream, move |mut raw_stream| {
                    async move {
                        loop {
                            if let Some(result) = raw_stream.next().await {
                                match result {
                                    Ok(message) => {
                                        let console_data: ConsoleData = ConsoleData {
                                            authcode: "0".to_string(),
                                            data: message.data,
                                            server: "unknown".into(),
                                            channel: "stdout".into(),
                                            message: "console".into(),
                                        };
                                        return Some((console_data, raw_stream));
                                    },
                                    Err(e) => {
                                        eprintln!("{:#?}", e);
                                        return None;
                                    }
                                }
                            }
                        }
                    }
                });
                *self.interface.console_out.lock().await = Some(stream.boxed());
                let _ = self.interface.active_event.send_replace(ConsoleEvent::Starting);
                Ok(())
            }
            Err(e) => {
                println!("{:#?}", e);
                Err("error".into())
            }
        }
    }
}

//

impl StreamTransportable for StartServerRequest {
    type Output = ();
    async fn stream_transport(
        &mut self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let state = arc_state.load();
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };

        let (server_in_tx, server_in_rx) = tokio::sync::mpsc::channel(32);
        let outbound_stream = ReceiverStream::new(server_in_rx);

        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        match clients.server_edit_client.clone().start(outbound_stream).await {
            Ok(response_stream) => {
                let raw_stream = response_stream.into_inner();

                let stream = stream::unfold(raw_stream, move |mut raw_stream| {
                    async move {
                        loop {
                            if let Some(result) = raw_stream.next().await {
                                match result {
                                    Ok(message) => {
                                        let console_data: ConsoleData = ConsoleData {
                                            authcode: "0".to_string(),
                                            data: message.data,
                                            server: "unknown".into(),
                                            channel: "stdout".into(),
                                            message: "console".into(),
                                        };
                                        return Some((console_data, raw_stream));
                                    },
                                    Err(e) => {
                                        eprintln!("{:#?}", e);
                                        return None;
                                    }
                                }
                            }
                        }
                    }
                });

                *self.interface.console_out.lock().await = Some(stream.boxed());
                *self.interface.console_in.lock().await = Some(server_in_tx);
                let _ = self.interface.active_event.send_replace(ConsoleEvent::Starting);

                Ok(())
            }
            Err(e) => {
                Err("error".into())
            }
        }
    }
}


impl NodeTransportable for StopServerRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };
        
        let stop_server_request = proto::StopServerRequest {};
        let _ = clients
            .server_edit_client
            .clone()
            .stop(stop_server_request)
            .await;

        Ok(())
    }
}

impl StreamTransportable for ConnectServerRequest {
    type Output = ();

    async fn stream_transport(
        &mut self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let state = arc_state.load();
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };

        let (server_in_tx, server_in_rx) = tokio::sync::mpsc::channel(32);
        let outbound_stream = ReceiverStream::new(server_in_rx);

        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        match clients.server_edit_client.clone().open(outbound_stream).await {
            Ok(response_stream) => {
                let raw_stream = response_stream.into_inner();

                let stream = stream::unfold(raw_stream, move |mut raw_stream| {
                    async move {
                        loop {
                            if let Some(result) = raw_stream.next().await {
                                match result {
                                    Ok(message) => {
                                        let console_data: ConsoleData = ConsoleData {
                                            authcode: "0".to_string(),
                                            data: message.data,
                                            server: "unknown".into(),
                                            channel: "stdout".into(),
                                            message: "console".into(),
                                        };
                                        return Some((console_data, raw_stream));
                                    },
                                    Err(e) => {
                                        eprintln!("{:#?}", e);
                                        return None;
                                    }
                                }
                            }
                        }
                    }
                });

                *self.interface.console_out.lock().await = Some(stream.boxed());
                *self.interface.console_in.lock().await = Some(server_in_tx);
                let _ = self.interface.active_event.send_replace(ConsoleEvent::Starting);

                Ok(())
            }
            Err(e) => {
                Err("error".into())
            }
        }
    }
}

// TODO: impliment this
impl NodeTransportable for MigrateRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // match serde_json::to_vec(&self.common) {
        //     Ok(bytes) => {
        //         if let Err(err) = state.connection_handler.tx.send(bytes) {
        //             eprintln!("Failed to send request over broadcast: {}", err);
        //         }
        //     }
        //     Err(err) => eprintln!("Failed to serialize request: {}", err),
        // }

        Ok(())
    }
}

impl NodeTransportable for SetServerRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        let set_server_request = proto::SetServerRequest {
            message: "set_server".to_string(),
            r#type: "command".to_string(),
            metadata: Some(self.metadata.clone().into()),
            authcode: "0".to_string(),
        };
        let _ = clients
            .server_manage_client
            .clone()
            .set(set_server_request)
            .await;

        Ok(())
    }
}
// NodeTransportable

impl NodeTransportable for ServerDataRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) = connection else {
            return Err("No connection working".into());
        };
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        let server_data_request = proto::ServerDataRequest {};
        let _ = clients
            .server_manage_client
            .clone()
            .data(server_data_request)
            .await;

        Ok(())
    }
}



// TODO: impliment this
impl NodeTransportable for FilterRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let filter_request = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_filter".to_string(),
            metadata: MetadataTypes::Filter(self.filter.clone()),
            authcode: "0".to_string(),
        };
        // let _ = state
        //     .connection_handler
        //     .tx
        //     .send(serde_json::to_vec(&filter_request).unwrap());

        Ok(())
    }
}


impl NodeTransportable for Ping {
    type Output = ();
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        let ping = SimpleMessage {
            message: "ping".to_string(),
        };
        // let res = state
        //     .connection_handler
        //     .tx
        //     .send(serde_json::to_vec(&ping).unwrap());

        Ok(())
    }
}

//InternalTransportable

// TODO: impliment this
impl NodeTransportable for IntegrationKeyRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        

        match serde_json::to_vec(&self.key) {
            Ok(mut bytes) => {
                // Add newline delimiter for TCP stream parsing
                bytes.push(b'\n');

                if let Err(err) = connection_handler.tx.send(bytes.clone()) {
                    eprintln!("Failed to send to internal stream: {}", err);
                }

                // Tells the remote server to enable RCON
                //if let Some(internal_tx) = &state.internal_tx {
                if let Err(err) = connection_handler.tx.send(bytes) {
                    eprintln!("Failed to send to TCP stream: {}", err);
                }
                //}
            }
            Err(err) => eprintln!("Failed to serialize request: {}", err),
        }

        Ok(())
    }
}

//InternalTransportable
impl NodeTransportable for ServerStateRequest {
    type Output = ServerStatus;
    async fn node_transport(
        &self,
        state: &AppState,
    ) -> Result<ServerStatus, Box<dyn Error + Send + Sync>> {
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };
        
        let server_state_request = proto::ServerStateRequest {};
        let result = clients
            .server_manage_client
            .clone()
            .state(server_state_request)
            .await;
        match result {
            Ok(res) => {
                let status = match res.get_ref().message.clone() {
                    0 => ServerStatus::Down,
                    1 => ServerStatus::Up,
                    2 => ServerStatus::Unknown,
                    3 => ServerStatus::Healthy,
                    4 => ServerStatus::Unhealthy,
                    _ => ServerStatus::Unknown
                };
                Ok(status)
                // let status_bool: bool = res.get_ref().message.clone().unwrap().message.parse()?;
                // if status_bool == true {
                //     Ok(Status::Up)
                // } else {
                //     Ok(Status::Down)
                // }
            }
            Err(e) => Err(Box::new(e)),
        }
    }
}

impl StreamTransportable for ServerStateRequest {
    type Output = watch::Receiver<ServerStatus>;

    async fn stream_transport(
        &mut self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        todo!()
    }
}



impl NodeTransportable for LsRequest {
    type Output = DirectoryResponse;
    async fn node_transport(&self, state: &AppState) -> Result<DirectoryResponse, Box<dyn Error + Send + Sync>> {
        Err("not implimented".into())
    }
}



impl StreamTransportable for FileUploadRequest {
    type Output = Arc<Notify>;
    async fn stream_transport(
        &mut self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let state = arc_state.load();
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        let (fs_tx, fs_rx) = tokio::sync::mpsc::channel(32);
        let outbound_stream = ReceiverStream::new(fs_rx);

        let location = self.file.location.clone();

        tokio::spawn(async move {

            if let Err(e) = clients.filesystem_client.clone().upload(outbound_stream).await  {
                println!("{:#?}", e)
            }
        });

        let stream = {
            let mut state = (*arc_state.load_full()).clone();
            state.filesystem.proxy_receiver().await
        };

        let end_of_file_task = Arc::new(Notify::new());

        let inner_end_of_file_task = end_of_file_task.clone();
        tokio::spawn(async move {
            while let Ok(bytes) = stream.recv_async().await {
                let res = fs_tx
                    .send(FileChunk {
                        location: location.clone(),
                        bytes,
                    })
                    .await;

                if res.is_err() {
                    break;
                }
            }
            inner_end_of_file_task.notify_waiters();
        });

        Ok(end_of_file_task)
    }
}

impl FileUploadRequest {
    pub fn new(location: String) -> FileUploadRequest {
        FileUploadRequest {
            file: RemoteFile {
                location,
                stream: None
            }
        }
    }
}

impl StreamTransportable for FileDownloadRequest {
    type Output = mpsc::Receiver<Vec<u8>>;
    async fn stream_transport(
        &mut self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let state = arc_state.load();
        let connection = state.current_node.connection.clone();
        let Some(connection_handler) =  connection else {
            return Err("No connection working".into());
        };
        let Some(clients) = connection_handler.clients.as_ref().clone() else {
            return Err("no clients".into())
        };

        let (fs_tx, fs_rx) = tokio::sync::mpsc::channel(32);

        let location = self.file.location.clone();
        tokio::spawn(async move {
            let request = DownloadRequest {
                location,
            };
            match clients.filesystem_client.clone().download(request).await {
                Ok(mut stream) => {
                    while let Some(Ok(chunk)) = stream.get_mut().next().await {
                        let _ = fs_tx.send(chunk.bytes).await;
                    }
                },
                Err(e) => {
                    println!("{:#?}", e);
                }
            }
        });
        Ok(fs_rx)
    }
}

impl FileDownloadRequest {
    pub fn new(location: String, task_end: Arc<CancellationToken>) -> FileDownloadRequest {
        FileDownloadRequest {
            file: RemoteFile {
                location,
                stream: None
            },
            task_end,
        }
    }
}

impl Into<proto::MetadataTypes> for MetadataTypes {
    fn into(self) -> proto::MetadataTypes {
        // TODO: remove hardcoding of server for
        // metadata conversion?
        match self {
            MetadataTypes::Server {
                servername,
                provider,
                providertype,
                location,
                sandbox,
                server_metadata,
            } => proto::MetadataTypes {
                kind: "Server".to_string(),
                data: serde_json::to_string(&MetadataTypes::Server {
                    servername: servername,
                    provider: provider,
                    providertype: providertype,
                    location: location,
                    sandbox: sandbox,
                    server_metadata: server_metadata,
                })
                .unwrap(),
            },
            _ => {
                println!("{:#?}", self);
                let value = serde_json::to_value(self.clone()).unwrap();
                println!("{:#?}", value);
                serde_json::from_value(value).unwrap()
            }
        }
    }
}


