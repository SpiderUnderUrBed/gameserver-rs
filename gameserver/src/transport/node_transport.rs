use dashmap::DashMap;
use general_networked_filesystem::core::Operation::Acknowlage;
use general_networked_filesystem::wrapper::FileSystemHandler;
use std::fs::File;
use std::io::{BufReader, Read};
use std::pin::Pin;
use std::{ops::ControlFlow, sync::Arc};
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use crate::{create_server_handler, start_server_handler, AppState, ErrorResponse, StreamResponse};
use async_trait::async_trait;
use futures::Stream;
use general_networked_filesystem::core::chain::ChainBuilder;
use general_networked_filesystem::core::{
    AcknowlageFrame, DrainFrame, EofFrame, FileFrame, FileHandleStatus, LocalState, SetFrame,
};

use serde::Serialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::{watch, Mutex},
};

use crate::{
    console_handler, delete_server_handler, server_data_handler, server_name_handler,
    server_state_handler, set_filter_handler, set_server_handler, stop_server_handler,
};
use crate::{
    ConsoleRequest, CreateServerRequest, DeleteServerRequest, ServerDataRequest, ServerNameRequest,
    ServerStateRequest, SetFilterRequest, SetServerRequest, StartServerRequest, StopServerRequest,
};

#[derive(Clone)]
pub struct Client {
    pub output_tx: Arc<Mutex<Option<mpsc::Sender<Vec<u8>>>>>,
    pub filesystem: Arc<RwLock<FileSystemHandler>>,
}

pub enum ClientEvent {
    None,
    Add(String),
    Remove(String)
}

#[derive(Clone)]
pub struct ClientManager {
    pub inner: DashMap<String, Client>,
    pub client_event: watch::Sender<ClientEvent>,
}
impl ClientManager {
    pub fn new() -> ClientManager {
        let (client_event_tx, _) = watch::channel(ClientEvent::None);
        ClientManager {
            inner: DashMap::new(),
            client_event: client_event_tx,
        }
    }
}

#[derive(Debug)]
pub enum BackgroundTaskUpdates {
    None,
    NoMoreFileTransfer,
}
// struct Test {

// }
// struct Test {

// }
pub async fn spawn_conn_background_tasks(
    arc_state: Arc<AppState>,
    arc_conn_manager: Arc<Mutex<ConnectionManager>>,
) {
    let (watch_tx, watch_rx) = watch::channel(BackgroundTaskUpdates::None);
    let mut conn_manager = arc_conn_manager.lock().await;
    conn_manager.backround_task_updates = Some(Arc::new(watch_rx));
    drop(conn_manager);

    tokio::spawn(async move {
        loop {
            let mut client_event_rx = arc_state.clients.client_event.subscribe();
            if client_event_rx.changed().await.is_err() {
                break;
            }

            let addr = {
                let event = client_event_rx.borrow();
                match &*event {
                    ClientEvent::Add(addr) => Some(addr.clone()),
                    ClientEvent::None => None,
                    ClientEvent::Remove(_) => None,
                }
            };

            let Some(addr) = addr else { continue };

            let Some(client) = arc_state.clients.inner.get(&addr) else {
                eprintln!("Client not found");
                continue;
            };
            let filesystem_lock = client.filesystem.clone();
            let output_tx = client.output_tx.lock().await.as_ref().unwrap().clone();
            drop(client);

            let watch_tx = watch_tx.clone();

            tokio::spawn(async move {
                let filesystem_reader = filesystem_lock.write().await;
                let mut file_rx = filesystem_reader.arc_file_tx.lock().await.clone();
                file_rx.create_state(
                    0,
                    LocalState {
                        location: "server/".to_owned(),
                    },
                );
                drop(filesystem_reader);

                let mut chain_builder = ChainBuilder::new(&mut file_rx);

                let arc_location = Arc::new(Mutex::new(String::new()));
                let inner_location = Arc::clone(&arc_location);
                let mut chain = chain_builder.chain::<FileFrame, _, _>(move |_, mut f, fs| {
                    let inner_location = inner_location.clone();
                    Box::pin(async move {
                        let location = inner_location.lock().await;
                        let _ = FileFrame::write_at_location(&mut f, fs, location.to_string());
                        Ok(())
                    })
                });

                let inner_watch_tx = watch_tx.clone();
                let arc_location_clone = Arc::clone(&arc_location);
                let mut chain = chain.chain::<SetFrame, _, _>(move |_, s, fs| {
                    Box::pin({
                        let inner_location = arc_location_clone.clone();
                        let inner_watch_tx = inner_watch_tx.clone();
                        async move {
                            let _ = inner_watch_tx.send(BackgroundTaskUpdates::None);
                            if let Ok(location_from_chunks) = String::from_utf8(s.chunks) {
                                *inner_location.lock().await = location_from_chunks;
                                Ok(())
                            } else {
                                Err(FileHandleStatus::IncorrectData)
                            }
                        }
                    })
                });

                let inner_out_tx = output_tx.clone();
                let mut chain = chain.chain::<EofFrame, _, _>(move |state_id, eof, fs| {
                    let inner_out_tx = inner_out_tx.clone();
                    Box::pin({
                        let inner_watch_tx = watch_tx.clone();
                        async move {
                            let _ = inner_watch_tx.send(BackgroundTaskUpdates::NoMoreFileTransfer);
                            let _ = EofFrame::handle::<_, _>(eof, state_id, fs).await;
                            if let Ok(bytes) = AcknowlageFrame::to_bytes_with_delims(fs.clone()) {
                                let _ = inner_out_tx.send(bytes).await;
                            } 
                            Ok(()) 
                        }
                    })
                });


                let mut chain = chain.chain::<DrainFrame, _, _>(move |state_id, drain, fs| {
                    Box::pin({
                        let inner_location = arc_location.clone();
                        let inner_out_tx = output_tx.clone();
                        async move {
                            let location = inner_location.lock().await;
                            let file = File::open(location.to_string())
                                .map_err(|e| FileHandleStatus::Any(Box::new(e)))?;

                            let mut reader = BufReader::new(file);
                            let mut chunk = vec![0u8; 1000];
                            let (tx, rx) = flume::unbounded();

                            tokio::task::spawn_blocking(move || {
                                loop {
                                    let n = reader.read(&mut chunk);
                                    match n {
                                        Ok(0) => {
                                            break;
                                        }
                                        Ok(n) => {
                                            if let Err(e) = tx.send(chunk[..n].to_vec()) {
                                                break;
                                            }
                                        }
                                        Err(e) => {
                                            println!("read error: {:#?}", e);
                                            break;
                                        }
                                    }
                                }
                            });
                            let mut rx_stream = rx.into_stream();
                            let fs_clone = fs.clone();
                            tokio::spawn(async move {
                                let _: Result<(), FileHandleStatus> = DrainFrame::write_from_custom_stream_with_delims(drain, state_id, fs_clone, &mut Some(&mut rx_stream), inner_out_tx).await;
                            });
                            Ok(())
                        }
                    })
                });

                loop {
                    if client_event_rx.has_changed().unwrap() {
                        if let ClientEvent::Remove(removing_addr) = &*client_event_rx.borrow() {
                            if *removing_addr == addr {
                                break;
                            }
                        }
                    }
                    if let Err(e) = chain.run(0).await {
                        println!("chain err: {:#?}", e);
                    }
                }
            });
        }
    });
    // Arc::new(watch_rx)
}

pub struct ConnectionManager {
    listner: TcpListener,
    backround_task_updates: Option<Arc<watch::Receiver<BackgroundTaskUpdates>>>,
}
impl ConnectionManager {
    pub async fn serve(
        _state: Arc<AppState>,
        url: String,
    ) -> Result<ConnectionManager, Box<dyn std::error::Error + Send + Sync>> {
        let listner = TcpListener::bind(url).await?;

        Ok(ConnectionManager {
            listner,
            backround_task_updates: None,
        })
    }
    pub async fn accept_connection(
        &mut self,
    ) -> Result<(ConnectionHandler, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
        let (socket, addr) = self.listner.accept().await?;
        let (tx, rx) = mpsc::unbounded_channel();
        let handler = ConnectionHandler {
            stream: Some(socket),
            read_buf: vec![],
            bytes_filter_method: BytesFilterMethod::Line,
            backround_task_updates: self.backround_task_updates.clone(),
            segments: vec![],
            shared_buf_chn: (tx, rx),
            addr: addr.clone().to_string(),
        };
        Ok((handler, Some(addr.to_string())))
    }
}

static FILE_STARTING_DELIMITER: &str = "\\\\f";

enum BytesFilterMethod {
    Line,
    All,
}

enum ProtocolMethod {
    Unknown,
    Json { distance: usize, inner_acc: usize },
}

enum Protocol {
    Json(usize),
    FileTransfer(usize),
    // Postcard,
    CobsWrapped(usize),
    Continue(usize),
}

pub struct ConnectionHandler {
    stream: Option<TcpStream>,
    shared_buf_chn: (
        mpsc::UnboundedSender<Vec<u8>>,
        mpsc::UnboundedReceiver<Vec<u8>>,
    ),
    read_buf: Vec<u8>,
    segments: Vec<String>,
    bytes_filter_method: BytesFilterMethod,
    backround_task_updates: Option<Arc<watch::Receiver<BackgroundTaskUpdates>>>,
    addr: String,
}

impl ConnectionHandler {
    pub fn inner(&mut self) -> &mut Vec<u8> {
        &mut self.read_buf
    }

    pub async fn start_clean_hook(&mut self) {
        self.remove_current_segment_or_clear().await;
    }
    pub async fn end_clean_hook(&mut self) {
        self.remove_current_segment_or_clear().await;
    }
    pub async fn remove_current_segment_or_clear(&mut self) {
        if self.segments.len() > 0 {
            let _ = self.segments.remove(0);
        }
    }
    fn remove_segment_or_clear(&mut self, position: usize) {
        if position + 1 <= self.inner().len() {
            self.inner().drain(..position + 1);
        } else {
            self.inner().clear();
        }
    }

    pub async fn next(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(backround_task_updates) = &self.backround_task_updates {
            if backround_task_updates.has_changed()? {
                if matches!(
                    *backround_task_updates.borrow(),
                    BackgroundTaskUpdates::NoMoreFileTransfer
                ) {
                    self.bytes_filter_method = BytesFilterMethod::Line;
                }
            }
        }

        if self.read_buf.is_empty() {
            println!("{} will be awaiting", self.addr);
            match self.shared_buf_chn.1.recv().await {
                Some(bytes) => {
                    println!("{} hhss", self.addr);
                    self.append_bytes(bytes).await
                }
                None => {
                    println!("{} returning", self.addr);
                    return Err("channel closed".into());
                }
            }
        } else {
            while let Ok(bytes) = self.shared_buf_chn.1.try_recv() {
                self.append_bytes(bytes).await;
            }
        }
        if matches!(self.bytes_filter_method, BytesFilterMethod::Line) {
            if self
                .read_buf
                .windows(FILE_STARTING_DELIMITER.len())
                .any(|bytes| bytes == FILE_STARTING_DELIMITER.as_bytes())
            {
                self.bytes_filter_method = BytesFilterMethod::All;
                return Ok(());
            }
            let read_buf_clone = self.read_buf.clone();
            let mut stream = serde_json::Deserializer::from_slice(&read_buf_clone)
                .into_iter::<serde_json::Value>();
            while let Some(Ok(value)) = stream.next() {
                self.segments.push(serde_json::to_string(&value).unwrap());
                if self.read_buf.len() >= stream.byte_offset() {
                    self.read_buf = self.read_buf[stream.byte_offset()..].to_vec();
                }
            }
            println!("returning from");
            Ok(())
        } else {
            Ok(())
        }
    }
    pub fn recv_bytes(&mut self) -> Vec<u8> {
        let bytes = self.read_buf.clone();
        self.read_buf = Vec::new();
        bytes
    }
    pub async fn recv_line(&mut self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if matches!(self.bytes_filter_method, BytesFilterMethod::All) {
            return Err("Cannot receive line when receiving all bytes".into());
        }

        let line = {
            if let Some(segment) = self.segments.pop() {
                segment
            } else {
                return Err("no line".into());
            }
        };

        self.remove_current_segment_or_clear().await;
        Ok(line)
    }
    pub async fn append_bytes(&mut self, bytes: Vec<u8>) {
        self.inner().extend_from_slice(&bytes);
    }


    pub fn split(&mut self) -> Result<(Writer, Reader), Box<dyn std::error::Error + Send + Sync>> {
        let stream = self.stream.take().ok_or("no stream set")?;
        let (read_half, write_half) = stream.into_split();
        Ok((
            Writer { write_half },
            Reader {
                read_half,
                shared_buf_chn_tx: self.shared_buf_chn.0.clone(),
            },
        ))
    }
}
pub struct Writer {
    write_half: OwnedWriteHalf,
}
impl Writer {
    pub async fn send(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.write_half.write_all(&bytes).await?;
        Ok(())
    }
}
pub struct Reader {
    read_half: OwnedReadHalf,
    shared_buf_chn_tx: mpsc::UnboundedSender<Vec<u8>>, //read_buf: Option<&Vec<u8>>,
}
impl Reader {
    pub async fn recv_into_buffer(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut temp_buf = vec![0u8; 4096];
        let n = match self.read_half.read(&mut temp_buf).await {
            Ok(n) => n,
            Err(_) => return Err("failed to read".into()),
        };
        if n == 0 {
            return Err("connection closed by peer or no bytes".into());
        }
        println!("sending it out");
        let _ = self.shared_buf_chn_tx.send(temp_buf[..n].to_vec());
        Ok(())
    }
}



#[async_trait]
#[typetag::serde(tag = "message")]
pub trait RequestByteExecutable: Send + Sync {
    async fn execute(&self, state: Arc<AppState>, addr: String) -> Vec<u8>;
}

#[async_trait]
#[typetag::serde(tag = "message")]
pub trait RequestStreamExecutable: Send + Sync {
    async fn execute_stream(
        &self,
        state: Arc<AppState>,
        addr: String,
    ) -> Result<StreamResponse<String>, ErrorResponse>;
}

#[async_trait]
#[typetag::serde(name = "server_name")]
impl RequestByteExecutable for ServerNameRequest {
    async fn execute(&self, state: Arc<AppState>, addr: String) -> Vec<u8> {
        serde_json::to_vec(&server_name_handler(&state, self.clone(), addr).await).unwrap()
    }
}

#[async_trait]
#[typetag::serde(name = "server_state")]
impl RequestByteExecutable for ServerStateRequest {
    async fn execute(&self, state: Arc<AppState>, addr: String) -> Vec<u8> {
        serde_json::to_vec(&server_state_handler(&state, self.clone(), addr).await).unwrap()
    }
}

#[async_trait]
#[typetag::serde(name = "server_data")]
impl RequestByteExecutable for ServerDataRequest {
    async fn execute(&self, state: Arc<AppState>, addr: String) -> Vec<u8> {
        serde_json::to_vec(&server_data_handler(&state, self.clone(), addr).await).unwrap()
    }
}

#[async_trait]
#[typetag::serde(name = "console")]
impl RequestByteExecutable for ConsoleRequest {
    async fn execute(&self, state: Arc<AppState>, addr: String) -> Vec<u8> {
        serde_json::to_vec(&console_handler(&state, self.clone(), addr).await).unwrap()
    }
}

#[async_trait]
#[typetag::serde(name = "set_filter")]
impl RequestByteExecutable for SetFilterRequest {
    async fn execute(&self, state: Arc<AppState>, addr: String) -> Vec<u8> {
        serde_json::to_vec(&set_filter_handler(&state, self.clone()).await).unwrap()
    }
}

#[async_trait]
#[typetag::serde(name = "set_server")]
impl RequestByteExecutable for SetServerRequest {
    async fn execute(&self, state: Arc<AppState>, addr: String) -> Vec<u8> {
        serde_json::to_vec(&set_server_handler(&state, self.clone(), addr).await).unwrap()
    }
}

#[async_trait]
#[typetag::serde(name = "delete_server")]
impl RequestByteExecutable for DeleteServerRequest {
    async fn execute(&self, state: Arc<AppState>, addr: String) -> Vec<u8> {
        serde_json::to_vec(&delete_server_handler(&state, self.clone()).await).unwrap()
    }
}

#[async_trait]
#[typetag::serde(name = "stop_server")]
impl RequestByteExecutable for StopServerRequest {
    async fn execute(&self, state: Arc<AppState>, addr: String) -> Vec<u8> {
        serde_json::to_vec(&stop_server_handler(&state, self.clone(), addr).await).unwrap()
    }
}

#[async_trait]
#[typetag::serde(name = "start_server")]
impl RequestStreamExecutable for StartServerRequest {
    async fn execute_stream(
        &self,
        state: Arc<AppState>,
        addr: String,
    ) -> Result<StreamResponse<String>, ErrorResponse> {
        start_server_handler(&state, self.clone(), addr).await
    }
}

#[async_trait]
#[typetag::serde(name = "create_server")]
impl RequestStreamExecutable for CreateServerRequest {
    async fn execute_stream(
        &self,
        state: Arc<AppState>,
        addr: String,
    ) -> Result<StreamResponse<String>, ErrorResponse> {
        create_server_handler(&state, self.clone(), addr).await
    }
}

// #[typetag::serde]
// impl RequestByteExecutable for StartServerRequest {
//     fn execute(&self, state: Arc<AppState>) -> Vec<u8> {

//     }
// }

// #[typetag::serde]
// impl RequestByteExecutable for CreateServerRequest {
//     fn execute(&self, state: Arc<AppState>) -> Vec<u8> {

//     }
// }

//StartServerRequest
//StopServerRequest
