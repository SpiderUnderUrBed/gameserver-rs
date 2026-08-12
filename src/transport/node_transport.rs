use axum::extract::{State, ws::Utf8Bytes};
use general_networked_filesystem::{FileRequestExecutable, LsRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::{broadcast, mpsc, RwLock},
    time::{sleep, timeout},
};
use tokio_util::sync::CancellationToken;

use crate::{database::{databasespec::{K8sType, NodeStatus, NodeType}, Element, ModifyElementData, Node, NodesDatabase}, extra::value_from_line, get_env_var_or_arg, kubernetes::{self, verify_is_k8s_gameserver}, ApiCalls as ToplevelApiCalls, AuthTcpMessage, Clients, ConsoleData, IncomingMessage, IntegrationCommands, List, LogLine, NodeAndTCP};
use crate::{
    AppState, CHANNEL_BUFFER_SIZE, CONNECTION_RETRY_DELAY, CONNECTION_TIMEOUT, MessagePayload,
    MessagePayloadWithMetadata, MetadataTypes, SimpleMessage, SrcAndDest, Status, StreamResult,
    database::databasespec::Filters,
};
use anyhow::anyhow;
use std::{
    error::Error,
    net::SocketAddr,
    sync::{atomic::{AtomicBool, Ordering}, Arc},
    time::{Duration, Instant},
};


pub struct PasswordRequest {
    pub password: String,
}
pub struct CapabilitiesRequest {
    pub capabilities: Vec<String>,
}
pub struct ServernameRequest {
    pub ip: String,
}
pub trait ImmediateTransportable {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}


impl ImmediateTransportable for PasswordRequest {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let auth_msg = serde_json::to_vec(&AuthTcpMessage {
            password: self.password.clone(),
        })?;
        let _ = state.connection_handler.proxy_tx.send(auth_msg);
        Ok(())
    }
}
impl ImmediateTransportable for CapabilitiesRequest {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let capability_msg = serde_json::to_vec(&List {
            list: ToplevelApiCalls::Capabilities(self.capabilities.clone()),
        })?;
        let _ = state.connection_handler.proxy_tx.send(capability_msg);
        Ok(())
    }
}

impl ImmediateTransportable for ServernameRequest {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let cmd_msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "server_name".to_string(),
            authcode: "0".to_string(),
        })?;
        let _ = state.connection_handler.proxy_tx.send(cmd_msg);
        // writer.write_all(cmd_msg.as_bytes()).await?;

        'name: {
            // let mut state = arc_state.write().await;
            if let Ok(Ok(bytes)) = timeout(
                Duration::from_millis(1000),
                state.connection_handler.rx.recv(),
            )
            .await
            {
                if let Ok(payload) = serde_json::from_slice::<IncomingMessage>(&bytes) {
                    state.current_node = NodeAndTCP {
                        name: payload.message,
                        ip: self.ip.clone(),
                        ..Default::default()
                    };
                    break 'name;
                }
            }
            state.current_node = NodeAndTCP {
                name: "main".to_string(),
                ip: self.ip.clone(),
                ..Default::default()
            };
        }

        Ok(())
    }
}


// What this does is that it will go over the lines retrived from the TCP stream
// and try parsing them into serveral objects, then it will put them in ConsoleData for it to be extracted and processed
// individually again
// (Sometimes the data sent is weird so this is why i do this intermediary step rather than directly processing things)
async fn get_all_stream_data_parsed(line_content: &str) -> Result<Vec<Value>, serde_json::Error> {
    let mut final_data = vec![];

    let list_parsed: Vec<Result<List, serde_json::Error>> =
        value_from_line::<List, _>(line_content, |line| line.contains("\"list\"")).await;

    let mut list_values: Vec<Value> = vec![];
    for item in list_parsed {
        if let Ok(list_item) = item {
            let serialized = serde_json::to_string(&list_item)?;
            if let Ok(seralized_value) = serde_json::to_value(ConsoleData {
                data: serialized,
                r#type: "list_item".to_string(),
                authcode: "0".to_string(),
            }) {
                if !list_values.contains(&seralized_value) {
                    list_values.push(seralized_value)
                }
            }
        }
    }
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
            if !list_lines.contains(&data.data) {
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

    if let Ok(value) = serde_json::from_str::<Value>(line_content) {
        if let (Some(_), Some(_), Some(_)) = (
            value.get("start_keyword").and_then(|v| v.as_str()),
            value.get("stop_keyword").and_then(|v| v.as_str()),
            value.get("name").and_then(|v| v.as_str()),
        ) {
            final_data.push(
                serde_json::to_value(ConsoleData {
                    authcode: "0".to_string(),
                    data: serde_json::to_string(&value).unwrap_or("".to_string()),
                    r#type: "info".to_string(),
                })
                .unwrap(),
            )
        }
    }

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
    return Ok(final_data);
}

// Deserializes all the values which has previously been serialized and processed individually
// rationale for this extra step is explained above
async fn handle_all_stream_values(
    arc_state: Arc<RwLock<AppState>>,
    value: Value,
    ws_tx: &broadcast::Sender<String>,
    ip: &str,
    server_start_keyword: &mut String,
    server_stop_keyword: &mut String,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    if let Ok(payload) = serde_json::from_value::<MessagePayload>(value.clone()) {
        //     if payload.message == "end_conn" {
        //         println!("Ending current connection");
        //         let mut state_guard = arc_state.write().await;
        //         state_guard.conn_status = Status::Down;
        //         return Ok(true);
        // } else
        if payload.r#type == "server_state" {
            let mut state_guard = arc_state.write().await;
            let sent_status = payload.message.parse().unwrap_or(false);
            state_guard.current_node.status = match sent_status {
                true => Status::Up,
                false => Status::Down,
            };
        }
    }

    if let Ok(data_clone) = serde_json::from_value::<SimpleMessage>(value.clone()) {
        if data_clone.message == "pong" {
            println!("got a ping");
            let state_guard: tokio::sync::RwLockWriteGuard<'_, AppState> = arc_state.write().await;
            let ping_message = MessagePayload {
                r#type: "status".to_string(),
                message: "ping_ok".to_string(),
                authcode: "0".to_string(),
            };
            let _ = state_guard
                .ws_tx
                .send(serde_json::to_string(&ping_message).unwrap());
        }
    }

    //println!("{:#?} and {:#?} end", serde_json::from_value::<ConsoleData>(value.clone()), value.clone());
    if let Ok(data_clone) = serde_json::from_value::<ConsoleData>(value.clone()) {
        if let Ok(inner_value) = serde_json::from_str::<serde_json::Value>(&data_clone.data) {
            if let (Some(start_kw), Some(stop_kw), Some(name)) = (
                inner_value.get("start_keyword").and_then(|v| v.as_str()),
                inner_value.get("stop_keyword").and_then(|v| v.as_str()),
                inner_value.get("name").and_then(|v| v.as_str()),
            ) {
                *server_start_keyword = start_kw.to_string();
                *server_stop_keyword = stop_kw.to_string();
                let mut state_guard = arc_state.write().await;
                if let Some(current_server) = &mut state_guard.current_server {
                    if current_server.servername.is_empty() {
                        current_server.servername = name.to_string();
                    }
                }
            }
        }

        if data_clone.data.contains("\"type\":\"command\"") {
            if let Ok(inner_msg) = serde_json::from_str::<MessagePayload>(&data_clone.data) {
                if inner_msg.r#type == "command" {
                    let (client_option, database) = {
                        let state_guard = arc_state.read().await;
                        (state_guard.client.clone(), state_guard.database.clone())
                    };

                    if let Ok(nodes) = database.fetch_all_nodes().await {
                        let node_status = if let Clients::K8s(client) = client_option {
                            let client_clone = client.clone();
                            let ip_clone = ip.to_string();
                            match tokio::time::timeout(
                                std::time::Duration::from_millis(100),
                                verify_is_k8s_gameserver(client_clone, ip_clone),
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
                                let state_guard = arc_state.read().await;
                                if let Clients::K8s(client) = &state_guard.client {
                                    if kubernetes::verify_is_k8s_gameserver(
                                        client.clone(),
                                        ip.to_string(),
                                    )
                                    .await?
                                    {
                                        NodeType::Inbuilt
                                    } else {
                                        NodeType::Custom
                                    }
                                } else {
                                    NodeType::Custom
                                }
                            },
                            nodestatus: node_status,
                            k8s_type: {
                                let state_guard = arc_state.read().await;
                                if let Clients::K8s(client) = &state_guard.client {
                                    if kubernetes::verify_is_k8s_gameserver(
                                        client.clone(),
                                        ip.to_string(),
                                    )
                                    .await?
                                    {
                                        if kubernetes::verify_is_k8s_pod(client, ip.to_string())
                                            .await?
                                        {
                                            K8sType::Pod
                                        } else if kubernetes::verify_is_k8s_node(
                                            client,
                                            ip.to_string(),
                                        )
                                        .await?
                                        {
                                            K8sType::Node
                                        } else {
                                            K8sType::Unknown
                                        }
                                    } else {
                                        K8sType::None
                                    }
                                } else {
                                    K8sType::Unknown
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


    Ok(false)
}

async fn process_stream_data(
    raw_data: &[u8],
    arc_state: &Arc<RwLock<AppState>>,
    ws_tx: &broadcast::Sender<String>,
    ip: &str,
    server_start_keyword: &mut String,
    server_stop_keyword: &mut String,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    if let Ok(text) = std::str::from_utf8(raw_data) {
        println!("got text {:#?}", text);
        let line_content = text.trim();
        if line_content.is_empty() {
            return Ok(false);
        }

        let final_data: Vec<Value> = get_all_stream_data_parsed(line_content).await?;

        //println!("{:#?}", final_data);

        for value in final_data.iter() {
            let should_break = handle_all_stream_values(
                arc_state.clone(),
                value.clone(),
                ws_tx,
                ip,
                server_start_keyword,
                server_stop_keyword,
            )
            .await?;
            if should_break {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub async fn node_start_hook(arc_state: Arc<RwLock<AppState>>, ip: String){
    let mut state = arc_state.write().await;
    let initial_node_password: String =
        get_env_var_or_arg("INITIAL_NODE_PASSWORD", Some(String::default())).unwrap();
    let password_request = PasswordRequest {
        password: initial_node_password,
    };
    let _ = password_request.immediate_transport(&mut state).await;

    let capability_request = CapabilitiesRequest {
        capabilities: vec!["all".to_string()],
    };
    let _ = capability_request.immediate_transport(&mut state).await;

    let server_name_request = ServernameRequest { ip: ip.clone() };
    let _ = server_name_request.immediate_transport(&mut state).await;
    drop(state);
    // TODO: consider interrupts instead of polling
    tokio::spawn(async move {
        let state = arc_state.write().await;
        let mut rx = state.cached_status_type.subscribe();
        drop(state);
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            let end_server_polling = AtomicBool::new(false);
            if rx.borrow().to_string() == "server-process" {
                let inner_arc_state = arc_state.clone();
                tokio::spawn(async move {
                    let state = inner_arc_state.read().await;
                    let notify = state.poll_server_event.clone();
                    drop(state);
                    let mut interval = tokio::time::interval(Duration::from_millis(500));
                    loop {
                        notify.notified().await;
                        if end_server_polling.load(Ordering::SeqCst) == true {
                            break;
                        }
                        let state = inner_arc_state.read().await;
                        let server_state_request = ServerStateRequest {};
                        let _ = server_state_request.node_transport(&state).await;
                        interval.tick().await;
                    }
                });
            } else {
                end_server_polling.store(true, Ordering::SeqCst);
            }
        }
    });
}

// This handles the stream
// when it should terminate and the handshake
// how internally it gets or sends from the stream
// is up to the implimentation, among other things
pub async fn handle_stream(
    arc_state: Arc<RwLock<AppState>>,
    rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    //stream: &mut TcpStream,
    ip: String,
    ws_tx: broadcast::Sender<String>,
) -> Result<StreamResult, Box<dyn std::error::Error + Send + Sync>> {

    let mut server_start_keyword = String::new();
    let mut server_stop_keyword = String::new();

    node_start_hook(arc_state.clone(), ip.clone()).await;

    let state = arc_state.read().await;
    let cloned_token = state.cancel_current_conn.clone();
    let mut internal_rx = state
        .internal_rx
        .as_ref()
        .map(|stream| stream.resubscribe());
    drop(state);

    loop {
        tokio::select! {
            Some(received) = async {
                match internal_rx.as_mut() {
                    Some(rx) => Some(rx.recv().await),
                    None => None,
                }
            }, if internal_rx.is_some() => {
                if let Ok(bytes) = received {
                    if process_stream_data(
                        &bytes, &arc_state, &ws_tx, &ip,
                        &mut server_start_keyword, &mut server_stop_keyword,
                    ).await? {
                        break;
                    }
                } else {
                    //break;
                }
            },
           broadcast_result = rx.recv() => {
            match broadcast_result {
                Ok(bytes) => {
                    println!("got bytes");
                    if process_stream_data(
                        &bytes, &arc_state, &ws_tx, &ip,
                        &mut server_start_keyword, &mut server_stop_keyword,
                    ).await? {
                        break;
                    }
                },
                Err(err) => {
                    println!("got err {:#?}", err);
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

    Ok(StreamResult::Done)
}


// does the connection to the tcp server, wether initial or not, on success it will pass it off to the dedicated handler for the stream
// Changelog:
// No more blocking with stream, if the caller wants the server connection to not be blocking its the callers job to spawn it in
// its own thread
pub async fn connect_to_server(
    arc_state: Arc<RwLock<AppState>>,
    tcp_url: String,
    ws_tx: broadcast::Sender<String>,
    end_if_timeout: bool,
) -> Result<Option<SocketAddr>, Box<dyn Error + Send + Sync>> {
    let mut last_peer: Option<SocketAddr> = None;
    //let (proxy_tx, _) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
    // let (proxy_tx, _) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
    // let (_, proxy_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
    let (proxy_tx, mut proxy_rx) = {
        let state_guard = arc_state.write().await;
        (
            state_guard.connection_handler.proxy_tx.clone(),
            state_guard.connection_handler.proxy_rx.resubscribe(),
        )
    };

    loop {
        let mut rx = {
            let state = arc_state.read().await;
            state.connection_handler.tx.subscribe()
        };
        let internal_stream = {
            let state = arc_state.read().await;
            state.internal_rx.as_ref().map(|r| r.resubscribe())
        };

        let deadline = Instant::now() + CONNECTION_TIMEOUT;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("overall connect timeout".into());
        }

        match timeout(remaining, TcpStream::connect(&tcp_url)).await {
            Ok(Ok(mut stream)) => {
                let peer = stream.peer_addr()?;
                last_peer = Some(peer);
                let cancel_token = CancellationToken::new();
                {
                    let mut state_guard = arc_state.write().await;
                    //state_guard.connection_handler.stream = Some(&stream);
                    state_guard.cancel_current_conn = cancel_token.clone();
                    state_guard.conn_status = Status::Up;
                }
                let ip = stream.peer_addr()?.ip().to_string();

                let (reader, mut writer) = stream.into_split();
                let buf_reader = BufReader::new(reader);
                let buf = vec![0u8; 4096];
                let mut lines = buf_reader.lines();

                let mut proxy_rx_clone = proxy_rx.resubscribe();
                let proxy_tx_clone = proxy_tx.clone();
                //let mut rx_clone = rx.resubscribe();

                //let arc_state_clone = arc_state.clone();

                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = cancel_token.cancelled() => {
                                let _ = writer.shutdown();
                                break;
                            },
                            read_result = lines.next_line() => {
                                match read_result {
                                    Ok(Some(line)) => {
                                        println!("Got line: {:#?}", line.clone());
                                        let bytes = line.as_bytes();
                                        let _ = proxy_tx_clone.send(bytes.to_vec());
                                    },
                                    Ok(None) => {
                                        break;
                                    }
                                    Err(e) => {
                                        break;
                                    },
                                }
                            }
                            rx = proxy_rx_clone.recv() => {
                                if let Ok(bytes) = rx {
                                    println!("got bytes to forward {:#?}", String::from_utf8(bytes.clone()).unwrap());
                                    if let Err(e) = writer.write_all(&bytes).await {
                                        println!("Error writing {}", e);
                                    }
                                    if let Err(e) = writer.write_all(b"\n").await {
                                        println!("Error writing {}", e);
                                    };
                                    if let Err(e) = writer.flush().await {
                                        println!("Error flushing writer: {}", e);
                                    };
                                }
                            }
                        }
                    }
                });

                let result = handle_stream(
                    Arc::clone(&arc_state),
                    &mut proxy_rx,
                    ip,
                    ws_tx.clone(),
                )
                .await;

                match result {
                    Ok(StreamResult::Reconnect(_, _)) => {}
                    Ok(StreamResult::Done) => {
                        return Ok(Some(last_peer.unwrap_or(peer)));
                    }
                    Err(e) => {
                        eprintln!("handle_stream error: {}", e);
                        // Fall through to retry delay
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("TCP connect error: {}", e);
                let mut state_guard = arc_state.write().await;
                if let Some(tx) = &state_guard.internal_tx {
                    let _ = tx.send("end_conn".into());
                }
                state_guard.conn_status = Status::Down;
            }
            Err(_) => {
                let mut state_guard = arc_state.write().await;
                state_guard.conn_status = Status::Down;
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
    state: &AppState,
    // tx: &broadcast::Sender<Vec<u8>>,
    // mut rx: broadcast::Receiver<Vec<u8>>,
) -> bool {
    let (tx, mut rx) = (
        state.connection_handler.proxy_tx.clone(),
        state.connection_handler.proxy_rx.resubscribe(),
    );
    match tx.send("ping".into()) {
        Ok(_) => true,
        Err(_) => return false,
    };

    match rx.recv().await {
        Ok(_msg) => true,
        Err(broadcast::error::RecvError::Closed) => false,
        Err(broadcast::error::RecvError::Lagged(_)) => true,
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
    state: &Arc<RwLock<AppState>>,
    tcp_url: String,
    ws_tx: &broadcast::Sender<String>,
    tx: tokio::sync::broadcast::Sender<Vec<u8>>,
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
                    let (_, temp_rx) =
                        tokio::sync::broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
                    let mut temp_rx = temp_rx;
                    let ip: String = stream.peer_addr()?.ip().to_string();

                    let stream_result =
                        handle_stream(state.clone(), &mut temp_rx, ip, ws_tx.clone()).await;
                    if stream_result.is_ok() {
                        println!("Stream finished");
                        return Ok(());
                    } else {
                        final_error = anyhow!(stream_result.err().unwrap())
                    }
                } else {
                    return Ok(());
                }
            }
            Err(e) => {
                eprintln!("Initial connection failed: {}", e);
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(final_error)
}

pub trait NodeTransportable {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>>;
}
impl NodeTransportable for LsRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut bytes = Vec::new();
        bytes.push(LsRequest::item_id());
        bytes.push(self.id);
        match serde_json::to_vec(&self) {
            Ok(b) => bytes.extend(b),
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        let _ = state.connection_handler.proxy_tx.send(bytes);
        Ok(())
    }
}

pub struct DeleteServerRequest {
    pub metadata: MetadataTypes,
}
// NodeTransportable
impl NodeTransportable for DeleteServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "delete_server".to_string(),
            authcode: "0".to_string(),
            metadata: self.metadata.clone(),
        };

        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');

        let _ = state.connection_handler.proxy_tx.send(bytes);

        Ok(())
    }
}

pub struct ConnectionHandler {
    //stream: Option<&'static TcpStream>,
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
        }
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
        }
    }
}
impl Clone for ConnectionHandler {
    fn clone(&self) -> Self {
        ConnectionHandler {
            //stream: None,
            proxy_tx: self.proxy_tx.clone(),
            proxy_rx: self.proxy_rx.resubscribe(),
            tx: self.tx.clone(),
            rx: self.rx.resubscribe(),
        }
    }
}

pub trait StreamTransportable {
    type Output;
    async fn stream_transport(
        &self,
        state: Arc<RwLock<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

pub struct CreateServerRequest {
    pub metadata: MetadataTypes,
}

impl StreamTransportable for CreateServerRequest {
    type Output = mpsc::Receiver<ConsoleData>;
    async fn stream_transport(
        &self,
        arc_state: Arc<RwLock<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "create_server".to_string(),
            metadata: self.metadata.clone(),
            authcode: "".to_string(),
        });
        if let Err(e) = msg {
            return Err("Failed to serialize".into());
        };
        let state = arc_state.write().await;
        let _ = state.connection_handler.proxy_tx.send(msg.unwrap());
        let mut proxy_rx = state.connection_handler.proxy_rx.resubscribe();
        drop(state);
        let (server_tx, server_rx) = tokio::sync::mpsc::channel(32);
        tokio::spawn(async move {
            loop {
                if let Ok(bytes) = proxy_rx.recv().await {
                    if let Ok(value) = serde_json::from_slice::<ConsoleData>(&bytes){
                        let _ = server_tx.send(value).await;
                    } 
                } else {
                    break;
                }
            }
        });


        Ok(server_rx)
    }
}

// NodeTransportable


pub struct StartServerRequest {
    // metadata: MetadataTypes
    pub stdin: Option<broadcast::Receiver<String>>
}
impl StreamTransportable for StartServerRequest {
    type Output = mpsc::Receiver<ConsoleData>;
    async fn stream_transport(
        &self,
        arc_state: Arc<RwLock<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "start_server".to_string(),
            authcode: "".to_string(),
        });
        if let Err(e) = msg {
            return Err("Failed to serialize".into());
        };
        let state = arc_state.write().await;
        let _ = state.connection_handler.proxy_tx.send(msg.unwrap());
        let mut proxy_rx = state.connection_handler.proxy_rx.resubscribe();
        drop(state);
        let (server_tx, server_rx) = tokio::sync::mpsc::channel(32);
        tokio::spawn(async move {
            loop {
                if let Ok(bytes) = proxy_rx.recv().await {
                    if let Ok(value) = serde_json::from_slice::<ConsoleData>(&bytes){
                        let _ = server_tx.send(value).await;
                    } 
                } else {
                    break;
                }
            }
        });


        Ok(server_rx)
    }
}



pub struct StopServerRequest {
    // metadata: MetadataTypes
}
impl NodeTransportable for StopServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "stop_server".to_string(),
            authcode: "".to_string(),
        });
        if let Err(e) = msg {
            return Err("Failed to serialize".into());
        };
        let _ = state.connection_handler.proxy_tx.send(msg.unwrap());

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct MigrateRequest {
    #[serde(flatten)]
    pub common: SrcAndDest,
}
impl NodeTransportable for MigrateRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::to_vec(&self.common) {
            Ok(bytes) => {
                if let Err(err) = state.connection_handler.proxy_tx.send(bytes) {
                    eprintln!("Failed to send request over broadcast: {}", err);
                }
            }
            Err(err) => eprintln!("Failed to serialize request: {}", err),
        }

        Ok(())
    }
}

pub struct SetServerRequest {
    pub(crate) metadata: MetadataTypes,
}
impl NodeTransportable for SetServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_server".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');
        let _ = state.connection_handler.proxy_tx.send(bytes);

        Ok(())
    }
}
// NodeTransportable

pub struct ServerDataRequest {
    pub(crate) metadata: MetadataTypes,
}
impl NodeTransportable for ServerDataRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "server_data".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');
        let _ = state.connection_handler.proxy_tx.send(bytes);

        Ok(())
    }
}

// struct ServerState
// NodeTransportable

// pub struct RawBytes {
//     pub(crate) bytes: Vec<u8>,
// }

// impl NodeTransportable for RawBytes {
//     async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
//         let _ = state.connection_handler.proxy_tx.send(self.bytes.clone());
//         Ok(())
//     }
// }
// NodeTransportable

trait InternalTransportable {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}
pub struct FilterRequest {
    //pub(crate) //metadata: MetadataTypes
    pub(crate) filter: Filters,
}
//InternalTransportable
// struct FilterRequest impl NodeTransportable {
impl NodeTransportable for FilterRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let filter_request = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_filter".to_string(),
            metadata: MetadataTypes::Filter(self.filter.clone()),
            authcode: "0".to_string(),
        };
        let _ = state
            .connection_handler
            .tx
            .send(serde_json::to_vec(&filter_request).unwrap());

        Ok(())
    }
}
impl InternalTransportable for FilterRequest {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

// TODO: add and associated type and generic for Ok type while the errors be an eum
trait ResultTransportable {
    async fn transport_and_recv(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}
// }
pub struct Ping {}
impl NodeTransportable for Ping {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let ping = SimpleMessage {
            message: "ping".to_string(),
        };
        let res = state
            .connection_handler
            .proxy_tx
            .send(serde_json::to_vec(&ping).unwrap());
        Ok(())
    }
}
impl ResultTransportable for Ping {
    async fn transport_and_recv(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
impl InternalTransportable for Ping {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
//InternalTransportable
pub struct IntegrationKeyRequest {
    pub key: Value,
}
impl NodeTransportable for IntegrationKeyRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::to_vec(&self.key) {
            Ok(mut bytes) => {
                // Add newline delimiter for TCP stream parsing
                bytes.push(b'\n');

                if let Err(err) = state.connection_handler.proxy_tx.send(bytes.clone()) {
                    eprintln!("Failed to send to internal stream: {}", err);
                }

                // Tells the remote server to enable RCON
                //if let Some(internal_tx) = &state.internal_tx {
                if let Err(err) = state.connection_handler.proxy_tx.send(bytes) {
                    eprintln!("Failed to send to TCP stream: {}", err);
                }
                //}
            }
            Err(err) => eprintln!("Failed to serialize request: {}", err),
        }

        Ok(())
    }
}
impl InternalTransportable for IntegrationKeyRequest {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
//InternalTransportable
pub struct ServerStateRequest {}
impl NodeTransportable for ServerStateRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "server_state".to_string(),
            authcode: "0".to_string(),
        })
        .unwrap();
        let _ = state.connection_handler.proxy_tx.send(msg);

        Ok(())
    }
}
impl InternalTransportable for ServerStateRequest {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

// NoteTransportable
//InternalTransportable
// struct SrcAndDestFsRequest {
//     src: String,
//     dest: String
// }
//InternalTransportable
// async fn send_request_to_node(){
// }
