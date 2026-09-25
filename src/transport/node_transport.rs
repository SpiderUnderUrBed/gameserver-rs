
use arc_swap::ArcSwap;
use dashmap::DashMap;
use general_networked_filesystem::core::{DirectoryResponse, FileRequest, LsRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::AsyncWriteExt, net::TcpStream, sync::{mpsc::{self}, watch, Mutex, RwLock}, time::{sleep, timeout}
};
use tokio_util::sync::CancellationToken;
use crate::{OrchestratorClients, ServerCheckEvent, StatusMethod, UserClient, transport::node_transport_spec::{CapabilitiesRequest, CreateServerRequest, DeleteServerRequest, FileDownloadRequest, FileUploadRequest, FilterRequest, IntegrationKeyRequest, MigrateRequest, NodeTransportable, NodeTransportableMut, Ping, RemoteFile, ServerDataRequest, ServerStateRequest, ServernameRequest, SetServerRequest, StartServerRequest, StateActionType, StopServerRequest, StreamTransportable, SwitchConsoleRequest}};
use crate::{
    ApiCalls as ToplevelApiCalls, AuthTcpMessage, ConsoleData, IncomingMessage,
    IntegrationCommands, KubeLocalRequest, List, LogLine, NodeWithConn,
    database::{
        Element, ModifyElementData, Node, NodesDatabase,
        databasespec::{K8sType, NodeStatus, NodeType},
    },
    extra::value_from_line,
    get_env_var_or_arg,
    kubernetes::{GetK8sTypeRequest, VerifyIsK8sGameserverRequest},
};
use crate::{
    AppState, CONNECTION_RETRY_DELAY, CONNECTION_TIMEOUT, MessagePayload,
    MessagePayloadWithMetadata, MetadataTypes, SimpleMessage, SrcAndDest, Status, StreamResult,
};
use std::{
    collections::HashMap, error::Error, sync::{
        Arc
    }, time::{Duration, Instant}
};
use tokio::io::AsyncReadExt;
use anyhow::anyhow;

pub struct PasswordRequest {
    pub password: String,
}

#[derive(Deserialize, Serialize)]
pub struct OkResponse {
    Ok: Value
}

impl NodeTransportable for ServernameRequest {
    type Output = NodeWithConn;

    async fn node_transport(&self, state: &AppState) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let bytes = convert_into_request(&MessagePayload {
            r#type: "command".to_string(),
            message: "server_name".to_string(),
            authcode: "0".to_string(),
        })?;

        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);
        // writer.write_all(cmd_msg.as_bytes()).await?;
        let (tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let share_tx_guard = state.connection_handler.share_tx.clone();
        // drop(state);
        let mut share_tx = share_tx_guard.lock().await;
        let index = share_tx.len();
        share_tx.insert(index, tx);
        drop(share_tx);

        if let Ok(Some(bytes)) = timeout(
            Duration::from_millis(1000),
            proxy_rx.recv(),
        )
        .await
        {
            if let Ok(payload) = serde_json::from_slice::<IncomingMessage>(&bytes) {
                return Ok(NodeWithConn {
                    name: payload.message,
                    ip: self.ip.clone(),
                    ..Default::default()
                });
            } else {
                return Ok(NodeWithConn {
                    name: "main".to_string(),
                    ip: self.ip.clone(),
                    ..Default::default()
                });
            }
        } else {
            return Ok(NodeWithConn {
                name: "main".to_string(),
                ip: self.ip.clone(),
                ..Default::default()
            });
        }
        

    }
}
impl NodeTransportable for CapabilitiesRequest {
    type Output = ();

    async fn node_transport(&self, state: &AppState) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let mut bytes = convert_into_request(&List {
            list: ToplevelApiCalls::Capabilities(self.capabilities.clone()),
        })?;
        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);
        Ok(())
    }
}
impl NodeTransportable for PasswordRequest {
    type Output = ();

    async fn node_transport(&self, state: &AppState) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let mut bytes = convert_into_request(&AuthTcpMessage {
            password: self.password.clone(),
        })?;

        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);
        Ok(())
    }
}

// What this does is that it will go over the lines retrived from the TCP stream
// and try parsing them into serveral objects, then it will put them in ConsoleData for it to be extracted and processed
// individually again
// (Sometimes the data sent is weird so this is why i do this intermediary step rather than directly processing things)
async fn get_all_stream_data_parsed(line_content: &str) -> Vec<Value> {
    let mut final_data = vec![];

    let list_parsed: Vec<Result<List, serde_json::Error>> =
        value_from_line::<List, _>(line_content, |line| line.contains("\"list\"")).await;

    let mut list_values: Vec<Value> = vec![];
    // for item in list_parsed {
    //     if let Ok(list_item) = item {
    //         if let Ok(serialized) = serde_json::to_string(&list_item) {
    //             if let Ok(seralized_value) = serde_json::to_value(ConsoleData {
    //                 data: serialized,
    //                 r#type: "list_item".to_string(),
    //                 authcode: "0".to_string(),
    //             }) {
    //                 if !list_values.contains(&seralized_value) {
    //                     list_values.push(seralized_value)
    //                 }
    //             }
    //         }
    //     }
    // }
    final_data.extend(list_values.clone());

    let list_lines: Vec<String> = list_values
        .iter()
        .map(|v| {
            serde_json::to_string(v.get("data").clone().unwrap_or(&Value::Null))
                .unwrap_or(String::new())
        })
        .collect();

    let console_parsed: Vec<Result<ConsoleData, serde_json::Error>> =
        value_from_line::<ConsoleData, _>(line_content, |line| !line.contains("\"list\"")).await;

    let mut console_values: Vec<Value> = vec![];

    for item in console_parsed {
        if let Ok(data) = item {
            if !list_lines.contains(&data.message) {
                if let Ok(seralized_value) = serde_json::to_value(data) {
                    console_values.push(seralized_value);
                }
            }
        }
    }
    final_data.extend(console_values);

    let console_parsed: Vec<Result<LogLine, serde_json::Error>> =
        value_from_line::<LogLine, _>(line_content, |line| !line.contains("\"list\"")).await;

    let mut console_values: Vec<Value> = vec![];
    for item in console_parsed {
        if let Ok(log) = item {
            if !list_lines.contains(&log.data) {
                console_values.push(serde_json::json!({ "data": log.data }));
            }
        }
    }
    final_data.extend(console_values);

    // if let Ok(value) = serde_json::from_str::<Value>(line_content) {
    //     if let (Some(_), Some(_), Some(_)) = (
    //         value.get("start_keyword").and_then(|v| v.as_str()),
    //         value.get("stop_keyword").and_then(|v| v.as_str()),
    //         value.get("name").and_then(|v| v.as_str()),
    //     ) {
    //         final_data.push(
    //             serde_json::to_value(ConsoleData {
    //                 authcode: "0".to_string(),
    //                 data: serde_json::to_string(&value).unwrap_or("".to_string()),
    //                 r#type: "info".to_string(),
    //                 server: todo!(),
    //                 channel: todo!(),
    //             })
    //             .unwrap(),
    //         )
    //     }
    // }

    let message_parsed: Vec<Result<MessagePayload, serde_json::Error>> =
        value_from_line::<MessagePayload, _>(line_content, |line| !line.contains("\"list\"")).await;

    let mut message_values: Vec<Value> = vec![];
    for item in message_parsed {
        if let Ok(data) = item {
            if let Ok(seralized_value) = serde_json::to_value(data) {
                if !message_values.contains(&seralized_value) {
                    message_values.push(seralized_value)
                }
            }
        }
    }
    final_data.extend(message_values);

    let src_and_dest_parsed: Vec<Result<SrcAndDest, serde_json::Error>> =
        value_from_line::<SrcAndDest, _>(line_content, |line| line.contains("\"src\"")).await;

    let mut src_and_dest_values: Vec<Value> = vec![];
    for item in src_and_dest_parsed {
        if let Ok(data) = item {
            if let Ok(serialized_value) = serde_json::to_value(data) {
                if !src_and_dest_values.contains(&serialized_value) {
                    src_and_dest_values.push(serialized_value);
                }
            }
        }
    }
    final_data.extend(src_and_dest_values);

    let simple_messages_parsed: Vec<Result<SimpleMessage, serde_json::Error>> =
        value_from_line::<SimpleMessage, _>(line_content, |line| line.contains("\"message\""))
            .await;

    let mut simple_messages_values: Vec<Value> = vec![];
    for item in simple_messages_parsed {
        if let Ok(data) = item {
            if let Ok(serialized_value) = serde_json::to_value(data) {
                if !simple_messages_values.contains(&serialized_value) {
                    simple_messages_values.push(serialized_value);
                }
            }
        }
    }
    final_data.extend(simple_messages_values);

    let integration_parsed: Vec<Result<IntegrationCommands, serde_json::Error>> =
        value_from_line::<IntegrationCommands, _>(line_content, |line| line.contains("\"kind\""))
            .await;

    let mut integration_values: Vec<Value> = vec![];
    for item in integration_parsed {
        if let Ok(data) = item {
            if let Ok(serialized_value) = serde_json::to_value(data) {
                if !integration_values.contains(&serialized_value) {
                    integration_values.push(serialized_value);
                }
            }
        }
    }

    final_data.extend(integration_values);
    return final_data;
}

// Deserializes all the values which has previously been serialized and processed individually
// rationale for this extra step is explained above
async fn handle_all_stream_values(
    arc_state: Arc<ArcSwap<AppState>>,
    value: Value,
    user_clients_option: Option<Arc<DashMap<i128, Arc<RwLock<UserClient>>>>>,
    ip: &str,
    server_start_keyword: &mut String,
    server_stop_keyword: &mut String,
) -> StreamResult {
    if let Ok(payload) = serde_json::from_value::<MessagePayload>(value.clone()) {

        if payload.r#type == "server_state" {
            println!("got a server state req");
            // if let Ok(mut state) = arc_state.try_write() {
            //     println!("wrote the state");
            //     let sent_status = payload.message.parse().unwrap_or(false);
            //     state.current_node.status = match sent_status {
            //         true => Status::Up,
            //         false => Status::Down,
            //     };
            // }
        }
    }

    if let Ok(data_clone) = serde_json::from_value::<SimpleMessage>(value.clone()) {
        if data_clone.message == "pong" {
            if let Some(arc_user_clients) = user_clients_option {
                // let state_guard: tokio::sync::RwLockWriteGuard<'_, AppState> = arc_state.write().await;
                let ping_message = MessagePayload {
                    r#type: "status".to_string(),
                    message: "ping_ok".to_string(),
                    authcode: "0".to_string(),
                };
                // let mut user_clients_lock = arc_user_clients.inner.lock().await;
                // let user_clients = user_clients_lock.deref_mut();
                // for (_, client_lock) in arc_user_clients.inner {
                //     let client = client_lock.write().await;
                //     while let Some(sender) = &client.server_connection {
                //         let _ = sender.send(serde_json::to_string(&ping_message).unwrap());
                //     }
                // }
            }
        }
    }

    //println!("{:#?} and {:#?} end", serde_json::from_value::<ConsoleData>(value.clone()), value.clone());
    if let Ok(data_clone) = serde_json::from_value::<ConsoleData>(value.clone()) {
        // if let Ok(inner_value) = serde_json::from_str::<serde_json::Value>(&data_clone.data) {
        //     if let (Some(start_kw), Some(stop_kw), Some(name)) = (
        //         inner_value.get("start_keyword").and_then(|v| v.as_str()),
        //         inner_value.get("stop_keyword").and_then(|v| v.as_str()),
        //         inner_value.get("name").and_then(|v| v.as_str()),
        //     ) {
        //         *server_start_keyword = start_kw.to_string();
        //         *server_stop_keyword = stop_kw.to_string();
        //         let mut state_guard = arc_state.write().await;
        //         if let Some(current_server) = &mut state_guard.current_server {
        //             if current_server.servername.is_empty() {
        //                 current_server.servername = name.to_string();
        //             }
        //         }
        //     }
        // }

        if data_clone.message.contains("\"type\":\"command\"") {
            if let Ok(inner_msg) = serde_json::from_str::<MessagePayload>(&data_clone.message) {
                if inner_msg.r#type == "command" {
                    let (client_option, database) = {
                        let state = arc_state.load();
                        (state.client.clone(), state.database.clone())
                    };

                    if let Ok(nodes) = database.fetch_all_nodes().await {
                        let node_status = if let OrchestratorClients::K8sLocal(client) = client_option {
                            // let client_clone = client.clone();
                            let ip_clone = ip.to_string();
                            let request = VerifyIsK8sGameserverRequest { server: ip_clone };
                            match tokio::time::timeout(
                                std::time::Duration::from_millis(100),
                                request.execute_locally(client.clone()),
                            )
                            .await
                            {
                                Ok(Ok(true)) => NodeStatus::ImmutablyEnabled,
                                _ => NodeStatus::Enabled,
                            }
                        } else {
                            NodeStatus::Enabled
                        };

                        let node = Node {
                            ip: ip.to_string(),
                            nodename: inner_msg.message,
                            nodetype: {
                                let state = arc_state.load();
                                if let OrchestratorClients::K8sLocal(client) = &state.client {
                                    let request = VerifyIsK8sGameserverRequest {
                                        server: ip.to_string(),
                                    };
                                    match request.execute_locally(client.clone()).await {
                                        Ok(is_gameserver) => {
                                            if is_gameserver {
                                                NodeType::Inbuilt
                                            } else {
                                                NodeType::Custom(None)
                                            }
                                        }
                                        Err(e) => return StreamResult::Error(e)
                                    } 
                                } else {
                                    NodeType::Custom(None)
                                }
                            },
                            nodestatus: node_status,
                            k8s_type: {
                                let state = arc_state.load();
                                if let OrchestratorClients::K8sLocal(client) = &state.client {
                                    let request = VerifyIsK8sGameserverRequest {
                                        server: ip.to_string(),
                                    };
                                    match request.execute_locally(client.clone()).await {
                                        Ok(is_gameserver) => {
                                            let request = GetK8sTypeRequest {
                                                server: ip.to_string(),
                                            };
                                            match request.execute_locally(client.clone()).await {
                                                Ok(k8s_type) => k8s_type,
                                                Err(e) => return StreamResult::Error(e),
                                            }
                                        }
                                        Err(e) => return StreamResult::Error(e)
                                    } 
                                } else {
                                    K8sType::None
                                }
                            },
                        };

                        if !nodes
                            .iter()
                            .any(|n| n.ip == node.ip && n.nodename == node.nodename)
                        {
                            let _ = database
                                .create_nodes_in_db(ModifyElementData {
                                    element: Element::Node(node),
                                    jwt: "".to_string(),
                                    require_auth: false,
                                })
                                .await;
                        }
                    }
                }
            }
        }
    }

    StreamResult::Active
}

async fn process_stream_data(
    raw_data: &[u8],
    arc_state: &Arc<ArcSwap<AppState>>,
    // ws_tx: &broadcast::Sender<String>,
    user_clients_option: Option<Arc<DashMap<i128, Arc<RwLock<UserClient>>>>>,
    ip: &str,
    server_start_keyword: &mut String,
    server_stop_keyword: &mut String,
) -> StreamResult {
    if let Ok(text) = std::str::from_utf8(raw_data) {
        let line_content = text.trim();
        if line_content.is_empty() {
            println!("line is empty");
            return StreamResult::Active;
        }

        println!("got text {:#?}", text);

        let final_data: Vec<Value> = get_all_stream_data_parsed(line_content).await;


        for value in final_data.iter() {
            let stream_values_result = handle_all_stream_values(
                arc_state.clone(),
                value.clone(),
                user_clients_option.clone(),
                ip,
                server_start_keyword,
                server_stop_keyword,
            )
            .await;
            println!("got a value");
            if matches!(stream_values_result, StreamResult::Done) || matches!(stream_values_result, StreamResult::Error(_)) {
                return stream_values_result;
            } 
        };
    } 
    StreamResult::Active
}

pub async fn node_start_hook(arc_state: Arc<ArcSwap<AppState>>, ip: String) {
    let state = arc_state.load();
    println!("got state for start hook");
    let initial_node_password: String =
        get_env_var_or_arg("INITIAL_NODE_PASSWORD", Some(String::default())).unwrap();
    let password_request = PasswordRequest {
        password: initial_node_password,
    };
    let _ = password_request.node_transport(&state).await;

    let capability_request = CapabilitiesRequest {
        capabilities: vec!["all".to_string()],
    };
    let _ = capability_request.node_transport(&state).await;

    let server_name_request = ServernameRequest { ip: ip.clone() };
    let mut state: AppState = (*arc_state.load_full()).clone();
    let node = server_name_request.node_transport(&state).await.unwrap();
    state.current_node = node;
    arc_state.store(Arc::new(state.clone()));

    // TODO: consider interrupts instead of polling
    tokio::spawn(async move {
        // let inner_arc_state = arc_state.clone();
        let server_check_event = arc_state.load().server_check_event.clone();
        loop {
            let current_server_check_event = (*server_check_event.borrow()).clone();
            if let ServerCheckEvent::Add(new_server) = current_server_check_event {
                let _ = server_check_event.send(ServerCheckEvent::None);
                println!("got an add event");
                let inner_server_check_event_rx = server_check_event.subscribe().clone();
                let inner_arc_state = arc_state.clone();
                tokio::spawn(async move {
                    'server_status_task: loop {
                        let inner_arc_state = inner_arc_state.clone();
                        let state = inner_arc_state.load();
                        let process_guard = state.server_processes.get(&new_server).unwrap();
                        let status_method = &process_guard.load().status_method.clone();
                        let mut status_method_rx = status_method.subscribe();
                        println!("starting outer loop 1");
                        println!("{:#?}", *status_method.borrow());
                        if matches!(*status_method.borrow(), StatusMethod::Poll) {
                            println!("doing on poll");
                            let mut interval = tokio::time::interval(Duration::from_millis(10000));
                            loop {
                                println!("starting inner loop");
                                let server_state_request = ServerStateRequest { };
                                if let Ok(status) = server_state_request.node_transport(&state).await{
                                    if process_guard.load().status != status {
                                        let mut updated_process = (*process_guard.load_full()).clone();
                                        updated_process.status = status;
                                        process_guard.store(Arc::new(updated_process));
                                    }
                                }
                                println!("sent the server state request");
                                if !matches!(*status_method.borrow(), StatusMethod::Poll){
                                    break;
                                }
                                if let ServerCheckEvent::Remove(server) = &*inner_server_check_event_rx.borrow() {
                                    if new_server == *server {
                                        break 'server_status_task;
                                    }
                                }
                                interval.tick().await;
                            }
                        } else if matches!(*status_method.borrow(), StatusMethod::OnUpdate){
                            println!("doing on update");
                            let server_state_request = ServerStateRequest {
                            };
                            if let Ok(rx) = server_state_request.stream_transport(inner_arc_state.clone()).await {
                                let inner_process_guard = process_guard.clone();
                                tokio::spawn(async move {
                                    loop {
                                        let status = rx.borrow().clone();
                                        if status != inner_process_guard.load().status {
                                            let mut inner_process = (*inner_process_guard.load_full()).clone();
                                            inner_process.status = status;
                                            inner_process_guard.store(Arc::new(inner_process));
                                        }   
                                    } 
                                });
                            }
                        }
                        if let ServerCheckEvent::Remove(server) = &*inner_server_check_event_rx.borrow() {
                            if new_server == *server {
                                break;
                            }
                        }
                        println!("before change");
                        let _ = status_method_rx.changed().await;
                        println!("changed");
                    }
                });
            }
            let _ = server_check_event.subscribe().changed().await;
        }
    });
}

// This handles the stream
// when it should terminate and the handshake
// how internally it gets or sends from the stream
// is up to the implimentation, among other things
pub async fn handle_stream(
    arc_state: Arc<ArcSwap<AppState>>,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    //stream: &mut TcpStream,
    ip: String,
    // ws_tx: broadcast::Sender<String>,
    user_clients_option: Option<Arc<DashMap<i128, Arc<RwLock<UserClient>>>>>
) -> StreamResult {
    let mut server_start_keyword = String::new();
    let mut server_stop_keyword = String::new();

    node_start_hook(arc_state.clone(), ip.clone()).await;

    let state = arc_state.load();
    let cloned_token = state.cancel_current_conn.clone();
    drop(state);
    
    loop {
        let inner_user_clients_option = user_clients_option.clone();
        tokio::select! {
           broadcast_result = rx.recv() => {
            match broadcast_result {
                Some(bytes) => {
                    println!("got bytes");
                    let stream_values_result = process_stream_data(
                        &bytes, &arc_state, inner_user_clients_option, &ip,
                        &mut server_start_keyword, &mut server_stop_keyword,
                    ).await;
                    if matches!(stream_values_result, StreamResult::Done) || matches!(stream_values_result, StreamResult::Error(_)) {
                        return stream_values_result;
                    } 
                    // let state = arc_state.write().await;

                    println!("finished prcoessing bytes");
                },
                None => {
                    println!("got err receiving");
                },
            }
           },
           _ = cloned_token.cancelled() => {
                // let state = arc_state.write().await;
                // let _ = state.connection_handler.shutdown().await;
                break;
            }
        }
    }

    StreamResult::Done
}

// does the connection to the tcp server, wether initial or not, on success it will pass it off to the dedicated handler for the stream
// Changelog:
// No more blocking with stream, if the caller wants the server connection to not be blocking its the callers job to spawn it in
// its own thread
pub async fn connect_to_server(
    arc_state: Arc<ArcSwap<AppState>>,
    tcp_url: String,
    user_clients: Arc<DashMap<i128, Arc<RwLock<UserClient>>>>,
    //ws_tx: broadcast::Sender<String>,
    end_if_timeout: bool,
) -> Result<watch::Receiver<StreamResult>, Box<dyn Error + Send + Sync>> {

    loop {
        let deadline = Instant::now() + CONNECTION_TIMEOUT;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("overall connect timeout".into());
        }

        match timeout(remaining, TcpStream::connect(&tcp_url)).await {
            Ok(Ok(stream)) => {
                // let peer = stream.peer_addr()?;
       
                let cancel_token = CancellationToken::new();
                
                let mut state: AppState = (*arc_state.load_full()).clone();
                state.cancel_current_conn = cancel_token.clone();
                state.conn_status = Status::Up;
                let ip = stream.peer_addr()?.ip().to_string();
                let (mut reader, mut writer) = stream.into_split();
                let share_tx_guard = state.connection_handler.share_tx.clone();
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
                state.connection_handler.proxy_tx = Some(tx);
                let arc_bytes_filter_method = state.connection_handler.byte_filter_method.clone();
                arc_state.store(Arc::new(state));

                let mut read_buf: Vec<u8> = Vec::new();
                let mut temp_buf = vec![0u8; 8192];

                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = cancel_token.cancelled() => {
                                let _ = writer.shutdown();
                                break;
                            },
                            read_res = reader.read(&mut temp_buf) => {
                                match read_res {
                                    Ok(0) => {
                                        println!("closed conn");
                                        break;
                                    },
                                    Ok(n) => {
                                        let bytes_filter_method = arc_bytes_filter_method.lock().await;
                                        match *bytes_filter_method {
                                            BytesFilterMethod::Line => {
                                                read_buf.extend_from_slice(&temp_buf[..n]);
                                                if read_buf.is_empty(){
                                                    continue;
                                                }
                                                while let Some(pos) = read_buf.windows(2).position(|b| b == "\n".as_bytes() || *b.last().unwrap() == b'\n'){
                                                    let full_segment: &Vec<u8> = &read_buf.drain(..=pos).collect();
                                                    let share_tx = share_tx_guard.lock().await;
                                                    for (_, tx) in share_tx.iter() {
                                                        let _ = tx.send(full_segment.clone());
                                                    }
                                                }
                                            },
                                            BytesFilterMethod::All => {
                                                let share_tx = share_tx_guard.lock().await;
                                                for (_, tx) in share_tx.iter() {
                                                    let _ = tx.send(temp_buf[..n].to_vec());
                                                }
                                            },
                                        }
                                    },
                                    Err(e) => {
                                        println!("Read error: {:#?}", e);
                                        break;
                                    },
                                }
                            }
  
                            receive_result = rx.recv() => {
                                if let Some(bytes) = receive_result {
                                    if let Ok(utf8_string) = String::from_utf8(bytes.clone()){
                                        println!("got bytes to forward {:#?}", utf8_string);
                                    }
                                    if let Err(e) = writer.write_all(&bytes).await {
                                        println!("Error writing {}", e);
                                    }

                                    if let Err(e) = writer.flush().await {
                                        println!("Error flushing writer: {}", e);
                                    };
                                }
                            }
                        }
                    }
                });

                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
                let state = arc_state.load();
                let shared_tx_guard = state.connection_handler.share_tx.clone();
                let mut share_tx = shared_tx_guard.lock().await;
                drop(state);
                let index = share_tx.len();
                share_tx.insert(index, tx);

                let (watch_tx, watch_rx) = watch::channel(StreamResult::Init);
                let inner_user_clients = user_clients.clone();
                tokio::spawn(async move {
                    let stream_result =
                        handle_stream(Arc::clone(&arc_state), &mut rx, ip, Some(inner_user_clients)).await;
                    
                    let _ = watch_tx.send(stream_result);
                });
                return Ok(watch_rx);
            }
            Ok(Err(e)) => {
                eprintln!("TCP connect error: {}", e);
                let mut state: AppState = (*arc_state.load_full()).clone();
                state.conn_status = Status::Down;
                arc_state.store(Arc::new(state));
                // arc_state.rcu(|state| {
                //     state.conn_status = Status::Down;
                //     state
                // });
            }
            Err(_) => {
                let mut state: AppState = (*arc_state.load_full()).clone();
                state.conn_status = Status::Down;
                arc_state.store(Arc::new(state));
                eprintln!("TCP connect timed out");
                if end_if_timeout {
                    return Err("connection attempt timed out".into());
                }
            }
        }

        sleep(CONNECTION_RETRY_DELAY).await;
    }
}

pub async fn check_channel_health(
    state: &mut AppState,
) -> bool {

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let mut share_tx = state.connection_handler.share_tx.lock().await;
    let index = share_tx.len();
    share_tx.insert(index, tx);
    if state.connection_handler.proxy_tx.is_none(){
        return false;
    }
    match state.connection_handler.proxy_tx.clone().unwrap().send("ping".into()) {
        Ok(_) => {},
        Err(_) => {
            share_tx.remove(&index);
            return false
        },
    };

    match rx.recv().await {
        Some(_msg) => {
            share_tx.remove(&index);
            true
        },
        None => {
            share_tx.remove(&index);
            false
        },
        // Err(broadcast::error::RecvError::Closed) => false,
        // Err(broadcast::error::RecvError::Lagged(_)) => true,
    }
}

// for the initial connection attempt, which will determine if possibly I would need to create the container and deployment upon failure
// i will use rusts 'timeout' for x interval determined with CONNECTION_TIMEOUT
async fn attempt_connection(
    tcp_url: String,
) -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    timeout(CONNECTION_TIMEOUT, TcpStream::connect(tcp_url))
        .await?
        .map_err(Into::into)
}

// this is where it determines wether or not to try and create the container and deployment, as attempt_connection itself is used in various diffrent contexts (like it will constantly
// try to connect upon failing but it should not try to create the container and deployment every time it fails)
// I use anyhow here because it saves me having to try and downcast the error type
pub(crate) async fn try_initial_connection(
    conn_attempts: u64,
    conn_timeout: u64,
    create_handler: bool,
    arc_state: &Arc<ArcSwap<AppState>>,
    tcp_url: String,
) -> Result<(), anyhow::Error> {
    let mut final_error = anyhow!(String::new());
    for _ in 0..conn_attempts {
        match attempt_connection(tcp_url.clone()).await {
            Ok(stream) => {
                println!("Initial connection succeeded!");
                // note, possibly I wont ever need to create a handler from the test of the intial connection
                // TODO: think about removing create_handler and just never create a handler here
                // I was considering to return the handler from here, but it wouldnt make sense to add that complexity
                // when I only create the initial tcp stream within the main function, it would involve either a thread here, or in the main function
                // and i rather keep this function focused on testing the connection (there might be a very NICHE case for making a handler here, but if there isnt ill remove it)
                if create_handler {

                    let state = arc_state.load();
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
                    let mut share_tx = state.connection_handler.share_tx.lock().await;
                    let index = share_tx.len();
                    share_tx.insert(index, tx);
                    drop(share_tx);

                    let ip: String = stream.peer_addr()?.ip().to_string();

                    let stream_result =
                        handle_stream(arc_state.clone(), &mut rx, ip, None).await;
                    if matches!(stream_result, StreamResult::Active) || matches!(stream_result, StreamResult::Done){
                        println!("Stream test successful");
                    } else {
                        if let StreamResult::Error(e) = stream_result {
                            final_error = anyhow!(e);
                        } else {
                            final_error = anyhow!("Stream ended unexpectedly: {:#?}", stream_result);
                        }
                    }
                } else {
                    return Ok(());
                }
            }
            Err(e) => {
                eprintln!("Initial connection failed: {}", e);
            }
        }
        tokio::time::sleep(Duration::from_secs(conn_timeout)).await;
    }
    Err(final_error)
}


pub enum BytesFilterMethod {
    Line, 
    All
}

#[derive(Clone)]
pub struct ConnectionHandler {
    //stream: Option<&'static TcpStream>,
    pub(crate) proxy_tx: Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>,
    pub(crate) share_tx: Arc<Mutex<HashMap<usize, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>>,
    pub(crate) current_active_priority: Arc<Mutex<usize>>,
    pub(crate) byte_filter_method: Arc<Mutex<BytesFilterMethod>>,
}
impl ConnectionHandler {
    pub fn new() -> Self {
        ConnectionHandler {
            //stream: None,
            proxy_tx: None,
            share_tx: Arc::new(Mutex::new(HashMap::new())),
            current_active_priority: Arc::new(Mutex::new(0)),
            byte_filter_method: Arc::new(Mutex::new(BytesFilterMethod::Line)),
        }
    }
}
impl Default for ConnectionHandler {
    fn default() -> Self {
        ConnectionHandler {
            //stream: None,
            proxy_tx: None,
            share_tx: Arc::new(Mutex::new(HashMap::new())),
            current_active_priority: Arc::new(Mutex::new(0)),
            byte_filter_method: Arc::new(Mutex::new(BytesFilterMethod::Line)),
        }
    }
}

fn convert_into_request<S: Serialize>(s: S) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let mut new_bytes = Vec::new();
    new_bytes.extend(&serde_json::to_vec(&s)?);
    Ok(new_bytes)
}

impl NodeTransportable for LsRequest {
    type Output = DirectoryResponse;
    async fn node_transport(&self, state: &AppState) -> Result<DirectoryResponse, Box<dyn Error + Send + Sync>> {
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        let file_req_traint: &dyn FileRequest = self; 
        let mut bytes = Vec::new();
        match serde_json::to_vec(&file_req_traint) {
            Ok(b) => bytes.extend(b),
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);
        let (tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let share_tx_guard = state.connection_handler.share_tx.clone();

        let mut share_tx = share_tx_guard.lock().await;
        let index = share_tx.len();
        share_tx.insert(index, tx);
        drop(share_tx);
        //let mut proxy_rx = state.connection_handler.proxy_rx.resubscribe();
        loop {
            if let Some(bytes) = proxy_rx.recv().await {
                if let Ok(response) = serde_json::from_slice::<DirectoryResponse>(&bytes) {
                    let mut share_tx = share_tx_guard.lock().await;
                    share_tx.remove(&index);
                    return Ok(response)
                }
            } else {
                return Err("Receiver failed".into());
            }
        }
    }
}

// NodeTransportable
impl NodeTransportable for DeleteServerRequest {
    type Output = ();
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "delete_server".to_string(),
            authcode: "0".to_string(),
            metadata: self.metadata.clone(),
        };

        let mut bytes = match convert_into_request(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };

        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);

        Ok(())
    }
}



impl StreamTransportable for CreateServerRequest {
    type Output = mpsc::Receiver<ConsoleData>;
    async fn stream_transport(
        &self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let mut bytes = convert_into_request(&MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "create_server".to_string(),
            metadata: self.metadata.clone(),
            authcode: "".to_string(),
        }).unwrap();

        let state = arc_state.load();
        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);
        let (tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let share_tx_guard = state.connection_handler.share_tx.clone();
        drop(state);
        let mut share_tx = share_tx_guard.lock().await;
        let index = share_tx.len();
        share_tx.insert(index, tx);
        drop(share_tx);

        let (server_tx, server_rx) = tokio::sync::mpsc::channel(32);
        let inner_console_task = self.active.clone();
        tokio::spawn(async move {
            loop {
                if let Some(bytes) = proxy_rx.recv().await {
                    if let Ok(value) = serde_json::from_slice::<ConsoleData>(&bytes) {
                        let _ = server_tx.send(value).await;
                        let mut share_tx = share_tx_guard.lock().await;
                        share_tx.remove(&index);
                    }
                } else {
                    break;
                }
                inner_console_task.cancel();
            }
        });

        Ok(server_rx)
    }
}


impl StreamTransportable for StartServerRequest {
    type Output = ();
    async fn stream_transport(
        &self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {

        let mut bytes = convert_into_request(&MessagePayload {
            r#type: "command".to_string(),
            message: "start_server".to_string(),
            authcode: "".to_string(),
        }).unwrap();
        // if let Err(_) = msg {
        //     return Err("Failed to serialize".into());
        // };
        println!("before state write lock");
        let state = arc_state.load();
        println!("after state write lock");

        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let proxy_tx = state.connection_handler.proxy_tx.clone().unwrap();
        let _ = proxy_tx.send(bytes);
        println!("after sending message");
        drop(state);
        
        let inner_arc_state = Arc::clone(&arc_state);
        // let (server_tx, server_rx) = tokio::sync::mpsc::channel(32);
        let mut stdin = self.stdin.resubscribe();

        let stdout = self.stdout.clone();
        let inner_console_task = self.active.clone();
        tokio::spawn(async move {
            let (tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
            let state = inner_arc_state.load();
            let share_tx_guard = state.connection_handler.share_tx.clone();
            let mut share_tx = share_tx_guard.lock().await;
            let index = share_tx.len();
            share_tx.insert(index, tx);
            drop(share_tx);
            loop {
                tokio::select! {
                    Some(bytes) = proxy_rx.recv() => {
                        if let Ok(value) = serde_json::from_slice::<ConsoleData>(&bytes) {
                            if let Err(_) = stdout.send(serde_json::to_string(&value).unwrap()) {
                                println!("User disconnected");
                                let mut share_tx = share_tx_guard.lock().await;
                                share_tx.remove(&index);
                                break;
                            }
                        }
                    }
                    Ok(msg) = stdin.recv() => {
                        let _ = proxy_tx.send(msg.into_bytes());
                    }
                }
                inner_console_task.cancel();
            }
        });

        Ok(())
        // Ok(server_rx)
    }
}

impl StreamTransportable for SwitchConsoleRequest {
    type Output = ();

    async fn stream_transport(
        &self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let state = arc_state.load();
        println!("after state write lock in switch console");

        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let proxy_tx = state.connection_handler.proxy_tx.clone().unwrap();
        println!("after sending message in switch console");
        drop(state);
        
        let inner_arc_state = Arc::clone(&arc_state);
        let mut stdin = self.stdin.resubscribe();
        let stdout = self.stdout.clone();
        tokio::spawn(async move {
            let (tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
            let state = inner_arc_state.load();
            let share_tx_guard = state.connection_handler.share_tx.clone();
            drop(state);
            let mut share_tx = share_tx_guard.lock().await;
            let index = share_tx.len();
            share_tx.insert(index, tx);
            drop(share_tx);
            loop {
                tokio::select! {
                    Some(bytes) = proxy_rx.recv() => {
                        if let Ok(value) = serde_json::from_slice::<ConsoleData>(&bytes) {
                            if let Err(_) = stdout.send(serde_json::to_string(&value).unwrap()) {
                                println!("User disconnected");
                                let mut share_tx = share_tx_guard.lock().await;
                                share_tx.remove(&index);
                                break;
                            }
                        }
                    }
                    Ok(msg) = stdin.recv() => {
                        let _ = proxy_tx.send(msg.into_bytes());
                    }
                }
            }
        });
        Ok(())
    }
}

impl NodeTransportable for StopServerRequest {
    type Output = ();
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "stop_server".to_string(),
            authcode: "".to_string(),
        });
        if let Err(_) = msg {
            return Err("Failed to serialize".into());
        };
        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(msg.unwrap());
        Ok(())
    }
}

impl NodeTransportable for MigrateRequest {
    type Output = ();
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        match serde_json::to_vec(&self.common) {
            Ok(bytes) => {
                if state.connection_handler.proxy_tx.is_none(){
                    return Err("no stream".into());
                }
                if let Err(err) = state.connection_handler.proxy_tx.clone().unwrap().send(bytes) {
                    eprintln!("Failed to send request over broadcast: {}", err);
                }
            }
            Err(err) => eprintln!("Failed to serialize request: {}", err),
        }

        Ok(())
    }
}

impl NodeTransportable for SetServerRequest {
    type Output = ();
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            // if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_server".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match convert_into_request(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };

        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);
        println!("sent set req");
        Ok(())
    }
}
// NodeTransportable

impl NodeTransportable for ServerDataRequest {
    type Output = ();
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "server_data".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match convert_into_request(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };

        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);

        Ok(())
    }
}


impl NodeTransportable for FilterRequest {
    type Output = ();
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        let filter_request = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_filter".to_string(),
            metadata: MetadataTypes::Filter(self.filter.clone()),
            authcode: "0".to_string(),
        };
        let _ = state
            .connection_handler
            .proxy_tx
            .clone()
            .unwrap()
            .send(serde_json::to_vec(&filter_request).unwrap());

        Ok(())
    }
}

// }

impl NodeTransportable for Ping {
    type Output = ();
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        println!("got ping request");
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            println!("high priority");
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        let ping = SimpleMessage {
            message: "ping".to_string(),
        };
        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        println!("sending ping");
        let _ = state
            .connection_handler
            .proxy_tx
            .clone()
            .unwrap()
            .send(serde_json::to_vec(&ping).unwrap());
        Ok(())
    }
}

impl NodeTransportable for IntegrationKeyRequest {
    type Output = ();
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        match serde_json::to_vec(&self.key) {
            Ok(mut bytes) => {
                if state.connection_handler.proxy_tx.is_none(){
                    return Err("no stream".into());
                }
                
                // Tells the remote server to enable RCON
                //if let Some(internal_tx) = &state.internal_tx {
                if let Err(err) = state.connection_handler.proxy_tx.clone().unwrap().send(bytes) {
                    eprintln!("Failed to send to TCP stream: {}", err);
                }
                //}
            }
            Err(err) => eprintln!("Failed to serialize request: {}", err),
        }

        Ok(())
    }
}

// #[derive(Deserialize, Serialize, Clone)]
// pub struct ServerStateRequest {
//     #[serde(flatten)]
//     pub common: IncomingMessage,
//     pub state_action: StateActionType
// }
impl NodeTransportable for ServerStateRequest {
    type Output = Status;
    async fn node_transport(&self, state: &AppState) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }

        let bytes = convert_into_request(&json!({
            "type": "command".to_string(),
            "message": "server_state".to_string(),
            "authcode": "0".to_string(),
            "state_action": &StateActionType::Immediate,
        }))
        .unwrap();

        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }
        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);

        let (tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let share_tx_guard = state.connection_handler.share_tx.clone();
        let mut share_tx = share_tx_guard.lock().await;
        let index = share_tx.len();
        share_tx.insert(index, tx);
        drop(share_tx);

        while let Some(bytes) = proxy_rx.recv().await {
            if let Ok(ok_response) = serde_json::from_slice::<OkResponse>(&bytes){
                if let Some(raw_status) = ok_response.Ok.get("message"){
                    
                    if let Ok(status) = serde_json::from_value::<Status>(raw_status.clone()){
                        println!("got a status");
                        return Ok(status);
                    }
                }
            }
        } 

        Err("Could not get the response".into())
    }
}

impl StreamTransportable for ServerStateRequest {
    type Output = watch::Receiver<Status>;

    async fn stream_transport(
        &self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let state = arc_state.load();
        if *state.connection_handler.current_active_priority.lock().await > 0 {
            return Err("high priority task is occuring and cant be interfered with".into())
        }
        drop(state);

        let state = arc_state.load();
        if state.connection_handler.proxy_tx.is_none(){
            return Err("no stream".into());
        }

        let mut bytes = convert_into_request(&json!({
            "type": "command".to_string(),
            "message": "server_state".to_string(),
            "authcode": "0".to_string(),
            "state_action": &StateActionType::OnUpdate,
        }))
        .unwrap();

        let _ = state.connection_handler.proxy_tx.clone().unwrap().send(bytes);
        let (tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let share_tx_guard = state.connection_handler.share_tx.clone();
        drop(state);
        let mut share_tx = share_tx_guard.lock().await;
        let index = share_tx.len();
        share_tx.insert(index, tx);
        drop(share_tx);

        let (watch_tx, watch_rx) = watch::channel(Status::Unknown);
        tokio::spawn(async move {
            // let mut previous_state: Status;
            while let Some(bytes) = proxy_rx.recv().await {
                if let Ok(payload) = serde_json::from_slice::<MessagePayload>(&bytes){
                    if let Ok(status_bool) = payload.message.parse::<bool>(){
                        let status =match status_bool {
                            true => Status::Up,
                            false => Status::Down,
                        };
                        let _ = watch_tx.send(status);
                    }
                }
            }
        });

        Ok(watch_rx)
    }
}

struct PriorityGuard {
    priority: Arc<tokio::sync::Mutex<usize>>,
}

impl Drop for PriorityGuard {
    fn drop(&mut self) {
        if let Ok(mut p) = self.priority.try_lock() {
            println!("resetting priority");
            *p = 0;
        } else {
            let priority = self.priority.clone();
            tokio::spawn(async move {
                println!("resetting priority here");
                *priority.lock().await = 0;
            });
        }
    }
}

struct BytesFilterMethodGuard {
    bytes_filter_method: Arc<tokio::sync::Mutex<BytesFilterMethod>>,
}

impl Drop for BytesFilterMethodGuard {
    fn drop(&mut self) {
        if let Ok(mut p) = self.bytes_filter_method.try_lock() {
            *p = BytesFilterMethod::Line;
        } else {
            let priority = self.bytes_filter_method.clone();
            tokio::spawn(async move {
                *priority.lock().await = BytesFilterMethod::Line;
            });
        }
    }
}

impl FileUploadRequest {
    pub fn new(request: (String, flume::Receiver<Vec<u8>>)) -> FileUploadRequest {
        FileUploadRequest {
            file: RemoteFile {
                location: request.0,
                stream: Some(request.1)
            }
        }
    }
}

impl StreamTransportable for FileUploadRequest {
    type Output = (flume::Receiver<Vec<u8>>, Arc<CancellationToken>);
    async fn stream_transport(
        &self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let state = arc_state.load();
        if state.connection_handler.proxy_tx.is_none() {
            return Err("no stream".into());
        }

        let (tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let share_tx_guard = state.connection_handler.share_tx.clone();
        let mut share_tx = share_tx_guard.lock().await;
        let index = share_tx.len();
        share_tx.insert(index, tx);
        drop(share_tx);
        let (flume_tx, flume_rx) = flume::bounded(32);
        
        let task_end = Arc::new(CancellationToken::new());

        let file_stream = self.file.stream.as_ref().unwrap().clone();
        let inner_task_end = task_end.clone();
        tokio::spawn(async move {
            let priority_handle = state.connection_handler.current_active_priority.clone();
            *priority_handle.lock().await = 1;

            drop(state);
            let _guard = PriorityGuard { priority: priority_handle };

            loop {
                tokio::select! {
                    biased;
                    Some(bytes) = proxy_rx.recv() => {
                        if let Err(_) = flume_tx.send_async(bytes).await {
                            break;
                        }
                    }
                    Ok(bytes) = file_stream.recv_async() => {
                        let tx = {
                            let state = arc_state.load();
                            state.connection_handler.proxy_tx.clone()
                        };
                        let Some(tx) = tx else {
                            println!("exiting file transfer");
                            // return Err("proxy_tx dropped mid-transfer".into());
                            break;
                        };
                        if let Err(e) = tx.send(bytes) {
                            println!("exiting file transfer");
                            // return Err("send failed mid-transfer".into());
                            break;
                        }
                    },
                    _ = inner_task_end.cancelled() => {
                        break;
                    }
                }
            }
            
        });

        Ok((flume_rx, task_end))
    }
}

impl StreamTransportable for FileDownloadRequest {
    // type Output = BoxStream<'static, Result<Vec<u8>, std::io::Error>>;
    type Output = flume::Receiver<Vec<u8>>;
    async fn stream_transport(
        &self,
        arc_state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let state = arc_state.load();
        if state.connection_handler.proxy_tx.is_none() {
            return Err("no stream".into());
        }
        let byte_filter_method_handle = state.connection_handler.byte_filter_method.clone();
        let priority_handle = state.connection_handler.current_active_priority.clone();
        *byte_filter_method_handle.lock().await = BytesFilterMethod::All;
        *priority_handle.lock().await = 1;
        drop(state);

        let inner_task_end = self.task_end.clone();
        tokio::spawn(async move {
            let _bytes_filter_method_guard = BytesFilterMethodGuard { bytes_filter_method: byte_filter_method_handle };
            let _priority_guard = PriorityGuard { priority: priority_handle };
            inner_task_end.cancelled().await;
        });

        let stream = self.file.stream.as_ref().unwrap().clone();
        let inner_arc_state = arc_state.clone();
        tokio::spawn(async move {
            while let Ok(bytes) = stream.recv_async().await {
                let tx = {
                    let state = inner_arc_state.load();
                    state.connection_handler.proxy_tx.clone()
                };
                if let Some(tx) = tx {
                    println!("sent message out");
                    if let Err(e) = tx.send(bytes){
                        println!("{:#?}", e);
                    }
                } else {
                    break;
                }
            }
        });

        let state = arc_state.load();
        let (tx, mut proxy_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let share_tx_guard = state.connection_handler.share_tx.clone();
        let mut share_tx = share_tx_guard.lock().await;
        let index = share_tx.len();
        share_tx.insert(index, tx);
        drop(share_tx);
        let (flume_tx, flume_rx) = flume::bounded(32);
        tokio::spawn(async move {
            while let Some(bytes) = proxy_rx.recv().await {
                let _ = flume_tx.send_async(bytes).await;
            }
        });
        Ok(flume_rx)
    }
}

impl FileDownloadRequest {
    pub fn new(file: (String, flume::Receiver<Vec<u8>>), task_end: Arc<CancellationToken>) -> FileDownloadRequest {
        FileDownloadRequest {
            file: RemoteFile {
                location: file.0,
                stream: Some(file.1),
            },
            task_end,
        }
    }
}