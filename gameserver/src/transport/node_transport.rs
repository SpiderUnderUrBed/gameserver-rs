use std::any::Any;

use file_transfer_system::server::Server;
use reqwest::Request;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tcp_filesystem::FileRequestMessage;

use crate::{IncomingMessage, IncomingMessageWithMetadata, SimpleMessage};

#[derive(Serialize, Deserialize, Clone)]
pub enum RequestTypes {
    Ping,
    CreateServerRequest(CreateServerRequest),
    DeleteSeverRequest(DeleteServerRequest),
    SetFilterRequest(SetFilterRequest),
    SetServerRequest(SetServerRequest),
    ServerDataRequest(ServerDataRequest),
    ServerNameRequest(ServerNameRequest),
    StartServerRequest(StartServerRequest),
    StopServerRequest(StopServerRequest),
    ServerStateRequest(ServerStateRequest),
    ConsoleRequest(ConsoleRequest),
    FileRequest(FileRequest),
}

pub struct RequestHandler {}
impl RequestHandler {
    pub fn try_recv_req(value: Value) -> Option<RequestTypes> {
        //if let Ok(ping_request)
        if let Ok(create_server_request) =
            serde_json::from_value::<CreateServerRequest>(value.clone())
        {
            if create_server_request.common.message == "create_server" {
                return Some(RequestTypes::CreateServerRequest(create_server_request));
            }
        }
        if let Ok(delete_server_request) =
            serde_json::from_value::<DeleteServerRequest>(value.clone())
        {
            if delete_server_request.common.message == "delete_server" {
                return Some(RequestTypes::DeleteSeverRequest(delete_server_request));
            }
        }
        if let Ok(set_server_request) = serde_json::from_value::<SetServerRequest>(value.clone()) {
            if set_server_request.common.message == "set_server" {
                return Some(RequestTypes::SetServerRequest(set_server_request));
            }
        }
        if let Ok(start_server_request) =
            serde_json::from_value::<StartServerRequest>(value.clone())
        {
            if start_server_request.common.message == "start_server" {
                return Some(RequestTypes::StartServerRequest(start_server_request));
            }
        }
        if let Ok(stop_server_request) = serde_json::from_value::<StopServerRequest>(value.clone())
        {
            if stop_server_request.common.message == "stop_server" {
                return Some(RequestTypes::StopServerRequest(stop_server_request));
            }
        }

        if let Ok(server_data_request) = serde_json::from_value::<ServerDataRequest>(value.clone())
        {
            if server_data_request.common.message == "server_data" {
                return Some(RequestTypes::ServerDataRequest(server_data_request));
            }
        }

        if let Ok(server_name_request) = serde_json::from_value::<ServerNameRequest>(value.clone())
        {
            if server_name_request.common.message == "server_name" {
                return Some(RequestTypes::ServerNameRequest(server_name_request));
            }
        }

        if let Ok(server_state_request) =
            serde_json::from_value::<ServerStateRequest>(value.clone())
        {
            if server_state_request.common.message == "server_state" {
                return Some(RequestTypes::ServerStateRequest(server_state_request));
            }
        }
        if let Ok(console_request) = serde_json::from_value::<ConsoleRequest>(value.clone()) {
            if console_request.common.message_type == "console" {
                return Some(RequestTypes::ConsoleRequest(console_request));
            }
        }
        if let Ok(ping_request) = serde_json::from_value::<SimpleMessage>(value.clone()) {
            if ping_request.message == "ping" {
                return Some(RequestTypes::Ping);
            }
        }
        if let Ok(set_filter_request) = serde_json::from_value::<SetFilterRequest>(value.clone()) {
            if set_filter_request.common.message == "set_filter" {
                return Some(RequestTypes::SetFilterRequest(set_filter_request));
            }
        }
        //println!("{:#?}", serde_json::from_value::<FileRequest>(value.clone()).iter().cloned());
        if let Ok(file_request) = serde_json::from_value::<FileRequest>(value.clone()) {
            println!("Got a file request");
            return Some(RequestTypes::FileRequest(file_request));
        }

        None
    }
}

impl TryIntoRequest for RequestTypes {
    type Request = Self;

    fn into_typed_request<T: 'static>(self) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let boxed: Box<dyn Any> = match self {
            RequestTypes::CreateServerRequest(req) => Box::new(req),
            RequestTypes::Ping => Box::new(Ping::default()),
            RequestTypes::DeleteSeverRequest(delete_server_request) => {
                Box::new(delete_server_request)
            }
            RequestTypes::SetFilterRequest(set_filter_request) => Box::new(set_filter_request),
            RequestTypes::SetServerRequest(set_server_request) => Box::new(set_server_request),
            RequestTypes::ServerDataRequest(server_data_request) => Box::new(server_data_request),
            RequestTypes::ServerNameRequest(server_name_request) => Box::new(server_name_request),
            RequestTypes::StartServerRequest(start_server_request) => {
                Box::new(start_server_request)
            }
            RequestTypes::StopServerRequest(stop_server_request) => Box::new(stop_server_request),
            RequestTypes::ServerStateRequest(server_state_request) => {
                Box::new(server_state_request)
            }
            RequestTypes::ConsoleRequest(console_request) => Box::new(console_request),
            RequestTypes::FileRequest(file_request) => Box::new(file_request),
        };

        boxed
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| "Type T did not match the inner request type".into())
    }

    fn into_request(
        value: Value,
    ) -> Result<Self::Request, Box<dyn std::error::Error + Send + Sync>> {
        Err("Not implimented".into())
    }
}

//FileRequestMessage
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct FileRequest {
    #[serde(flatten)]
    pub common: FileRequestMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ConsoleRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerStateRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StopServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StartServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerNameRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerDataRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct CreateServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DeleteServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SetServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SetFilterRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct Ping {
    #[serde(flatten)]
    pub common: SimpleMessage,
}

pub trait TryIntoRequest {
    type Request: DeserializeOwned + Serialize;

    fn into_typed_request<T: 'static>(self) -> Result<T, Box<dyn std::error::Error + Send + Sync>>;

    fn into_request(
        value: Value,
    ) -> Result<Self::Request, Box<dyn std::error::Error + Send + Sync>>;
}

// if let Ok(request) = serde_json::from_value::<SimpleMessage>(
//     json_value.clone(),
// ) {
//     if request.message == "ping" {
//         //let out_tx_clone = out_tx.clone();
//         let pong = SimpleMessage {
//             message: "pong".to_string(),
//         };
//         let _ = out_tx
//             .send(serde_json::to_string(&pong).unwrap())
//             .await;
//     }
// }
// if let Ok(request) =
//     serde_json::from_value::<FileRequestMessage>(
//         json_value.clone(),
//     )
// {
//     let out_tx_clone = out_tx.clone();
//     let arc_state_for_spawn = arc_state_clone.clone();
//     tokio::spawn(async move {
//         let response_json = handle_file_request(
//             &Arc::clone(&arc_state_for_spawn),
//             request,
//         )
//         .await;
//         let _ = out_tx_clone.send(response_json).await;
//     });
//     } else if let Ok(msg_payload) =
//         serde_json::from_value::<IncomingMessageWithMetadata>(
//             json_value.clone(),
//         )
//     {
//         println!(
//             "[{}] DEBUG: Processing command with metadata: {}",
//             addr, msg_payload.message
//         );
//         let _ = handle_commands_with_metadata(
//             arc_state_clone.clone(),
//             &msg_payload,
//             &cmd_tx,
//             &stdin_ref,
//             &hostname_ref,
//         )
//         .await;
//         if newline_pos + 1 <= read_buf.len() {
//             read_buf.drain(..newline_pos + 1);
//             found_message = true;
//         } else {
//             read_buf.clear();
//         }
//         continue;
//     } else if let Ok(payload) =
//         serde_json::from_value::<SrcAndDest>(json_value.clone())
//     {
//         if let ApiCalls::Node(dest) = payload.dest {
//             match unsure_ip_or_port_tcp_conn(
//                 Some(dest.ip.clone()),
//                 None,
//             )
//             .await
//             {
//                 Ok(conn) => {
//                     let writer_tx = tcp_to_writer(conn).await;
//                     tokio::spawn(async move {
//                         let _ = send_folder_over_broadcast(
//                             SERVER_DIR.to_string(),
//                             writer_tx,
//                         )
//                         .await;
//                     });
//                 }
//                 Err(e) => eprintln!(
//                     "[{}] Failed to connect: {}",
//                     addr, e
//                 ),
//             }
//         } else {
//             let _ = sort_command_type_or_console(
//                 &Arc::clone(&arc_state_clone),
//                 &json_value,
//                 &out_tx,
//                 &cmd_tx,
//                 &stdin_ref,
//                 &hostname_ref,
//             )
//             .await;
//         }
//     } else if let Ok(msg_payload) =
//         serde_json::from_value::<MessagePayload>(
//             json_value.clone(),
//         )
//     {
//         match msg_payload.r#type.as_str() {
//             "start_file" => {
//                 // TODO: consider whether or not to remove the file counter
//                 // files_received += 1;
//                 println!(
//                     "[File Transfer] {} is being transferred",
//                     msg_payload.message
//                 );
//                 let file_path = format!(
//                     "{}/{}",
//                     SERVER_DIR, msg_payload.message
//                 );
//                 let _ = tokio::fs::create_dir_all(
//                     file_path.clone(),
//                 )
//                 .await;

//                 if let Some(parent) =
//                     std::path::Path::new(&file_path).parent()
//                 {
//                     let _ =
//                         tokio::fs::create_dir_all(parent).await;
//                 }

//                 if let Ok(file) = tokio::fs::OpenOptions::new()
//                     .create(true)
//                     .write(true)
//                     .truncate(true)
//                     .open(&file_path)
//                     .await
//                 {
//                     mode = ReadMode::File {
//                         current_file: file,
//                         file_name: msg_payload.message.clone(),
//                         bytes_written: 0,
//                         last_logged_mb: 0,
//                         last_activity:
//                             tokio::time::Instant::now(),
//                     };
//                     if newline_pos + 1 <= read_buf.len() {
//                         read_buf.drain(..newline_pos + 1);
//                     } else {
//                         read_buf.clear();
//                     }
//                     found_message = true;
//                     break;
//                 }
//             }
//             "end_file" => {
//                 if newline_pos + 1 <= read_buf.len() {
//                     read_buf.drain(..newline_pos + 1);
//                 } else {
//                     read_buf.clear();
//                 }
//                 found_message = true;
//                 continue;
//             }
//             "clean_file" => {
//                 let file_path = format!(
//                     "{}/{}",
//                     SERVER_DIR, msg_payload.message
//                 );
//                 if tokio::fs::metadata(&file_path).await.is_ok()
//                 {
//                     let _ = cleanup_end_file_markers(
//                         &file_path,
//                         &msg_payload.message,
//                     )
//                     .await;
//                 }
//                 if newline_pos + 1 <= read_buf.len() {
//                     read_buf.drain(..newline_pos + 1);
//                 } else {
//                     read_buf.clear();
//                 }
//                 found_message = true;
//                 continue;
//             }
//             "command" => {
//                 let current_server_lock = arc_state_clone
//                     .current_server
//                     .lock()
//                     .await
//                     .clone();
//                 if msg_payload.message == "start_server" {
//                     println!("Called start server");
//                     let sandbox = {
//                         if let Some(ProviderTypes::Sandbox(
//                             sandbox,
//                         )) = convert_provider(
//                             arc_state_clone.clone(),
//                             vec![ProviderTypes::Name(
//                                 current_server_lock
//                                     .clone()
//                                     .unwrap_or(String::new()),
//                             )],
//                             ProviderReturnTypes::Sandbox,
//                         )
//                         .await
//                         {
//                             sandbox
//                         } else {
//                             false
//                         }
//                     };
//                     let option_path = {
//                         if let Some(ProviderTypes::Path(path)) =
//                             convert_provider(
//                                 arc_state_clone.clone(),
//                                 vec![ProviderTypes::Name(
//                                     current_server_lock
//                                         .clone()
//                                         .unwrap_or(
//                                             String::new(),
//                                         ),
//                                 )],
//                                 ProviderReturnTypes::Path,
//                             )
//                             .await
//                         {
//                             Some(path)
//                         } else {
//                             None
//                         }
//                     };

//                     println!("start_server: option_path = {:?}, sandbox = {}", option_path, sandbox);
//                     if let Err(e) = start_server_with_broadcast(
//                         &arc_state_clone,
//                         &stdin_ref,
//                         &cmd_tx,
//                         sandbox,
//                         option_path.unwrap_or(String::new()),
//                     )
//                     .await
//                     {
//                         eprintln!(
//                             "[{}] Failed to start server: {}",
//                             addr, e
//                         );
//                     }
//                 } else {
//                     let _ = sort_command_type_or_console(
//                         &Arc::clone(&arc_state_clone),
//                         &serde_json::to_value(msg_payload)
//                             .unwrap(),
//                         &out_tx,
//                         &cmd_tx,
//                         &stdin_ref,
//                         &hostname_ref,
//                     )
//                     .await;
//                 }
//             }
//             _ => {
//                 let _ = sort_command_type_or_console(
//                     &Arc::clone(&arc_state_clone),
//                     &serde_json::to_value(msg_payload).unwrap(),
//                     &out_tx,
//                     &cmd_tx,
//                     &stdin_ref,
//                     &hostname_ref,
//                 )
//                 .await;
//             }
//         }
//     } else {
//         // This is when there is no match for any existing data structure
//         let command_or_console_result =
//             sort_command_type_or_console(
//                 &Arc::clone(&arc_state_clone),
//                 &json_value,
//                 &out_tx,
//                 &cmd_tx,
//                 &stdin_ref,
//                 &hostname_ref,
//             )
//             .await;
//         if let Err(e) = command_or_console_result {
//             if let Some(
//                 CommandOrConsoleErrors::AuthDisconnect,
//             ) = e.downcast_ref::<CommandOrConsoleErrors>()
//             {
//                 println!("Killing connection");
//                 kill_socket = true;
//             }
//         }
//     }
