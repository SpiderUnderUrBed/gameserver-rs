use futures::future::pending;
use futures::Stream;
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tonic::Streaming;

use crate::transport::node_transport::proto::node_manage_server::NodeManage;
use crate::transport::node_transport::proto::server_manage_server::{
    ServerManage, ServerManageServer,
};
use crate::transport::node_transport::proto::ServerMessage;
use crate::{AppState, IncomingMessage, IncomingMessageWithMetadata};
use network_abstraction_lib::RouterErrors;
use network_abstraction_lib::{ExtractorErrors, Router};
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
mod node_transport_spec;
mod proto {
    tonic::include_proto!("main");
}
use proto::{server_edit_server::ServerEdit, server_edit_server::ServerEditServer};

use crate::transport::node_transport::node_transport_spec::ConsoleRequest;
use crate::transport::node_transport::node_transport_spec::CreateServerRequest;
use crate::transport::node_transport::node_transport_spec::DeleteServerRequest;
use crate::transport::node_transport::node_transport_spec::ServerDataRequest;
use crate::transport::node_transport::node_transport_spec::ServerNameRequest;
use crate::transport::node_transport::node_transport_spec::ServerStateRequest;
use crate::transport::node_transport::node_transport_spec::SetServerRequest;
use crate::transport::node_transport::node_transport_spec::StartServerRequest;
use crate::transport::node_transport::node_transport_spec::StopServerRequest;

pub enum BackgroundTaskUpdates {
    NoMoreFileTransfer
}

pub async fn spawn_conn_background_tasks(_arc_state: Arc<AppState>){
}
pub struct ConnectionManager {
    url: String,
    accepted_connection: bool,
    router: Arc<Mutex<Router<Arc<AppState>>>>,
}
impl ConnectionManager {
    pub async fn serve(
        router: Router<Arc<AppState>>,
        url: String,
    ) -> Result<ConnectionManager, Box<dyn std::error::Error + Send + Sync>> {
        Ok(ConnectionManager {
            accepted_connection: false,
            url,
            router: Arc::new(Mutex::new(router)),
        })
    }
    pub async fn accept_connection(
        &mut self,
    ) -> Result<(ConnectionHandler, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
        if !self.accepted_connection {
            self.accepted_connection = true;
        } else {
            let _never: () = pending().await;
        }
        let connection = Connection {
            router: Arc::clone(&self.router),
        };
        let handler = ConnectionHandler {
            // current_request: None,
            // requests: vec![],
            connection,
        };

        let inner_connection = Connection {
            router: Arc::clone(&self.router),
        };
        let inner_url = self.url.clone();
        tokio::spawn(async move {
            let _ = Connection::serve_with_arc(Arc::new(inner_connection), inner_url).await;
        });
        Ok((handler, None))
    }
    pub async fn get_arc_mutex_router(&self) -> Arc<Mutex<Router<Arc<AppState>>>> {
        self.router.clone()
    }
}

pub struct Connection {
    router: Arc<Mutex<Router<Arc<AppState>>>>,
}
#[derive(Clone)]
pub struct Request {
    data: String,
    result_tx: broadcast::Sender<String>,
}

#[tonic::async_trait]
impl ServerEdit for Connection {
    type StartStream = ReceiverStream<Result<ServerMessage, tonic::Status>>;
    type CreateStream = ReceiverStream<Result<ServerMessage, tonic::Status>>;
    async fn create(
        &self,
        request: tonic::Request<proto::CreateServerRequest>,
    ) -> std::result::Result<tonic::Response<Self::CreateStream>, tonic::Status> {
        // //let inner = request.into_inner();
        let (tx, rx) = mpsc::channel(32);

        let create_server_request = CreateServerRequest {
            common: IncomingMessageWithMetadata {
                metadata: request.get_ref().clone().metadata.unwrap().into(),
                message: "create_server".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
        };
        let mut router = self.router.lock().await;
        let response_result = router
            .execute_handler_typed(create_server_request, "create_server".to_string())
            .await;
        match response_result {
            Ok(response) => {
                match response.try_into_response() {
                    Ok(boxed) => match boxed
                        .downcast::<Pin<Box<dyn Stream<Item = String> + Send + Sync>>>()
                    {
                        Ok(stream_box) => {
                            println!("got a stream");
                            let mut stream = *stream_box;
                            tokio::spawn(async move {
                                while let Some(message) = stream.next().await {
                                    let _ = tx
                                        .send(Ok(ServerMessage {
                                            authcode: "0".to_string(),
                                            data: message,
                                            r#type: "command".to_string(),
                                        }))
                                        .await;
                                }
                                let _ = tx.send(Err(tonic::Status::aborted("stream EOF"))).await;
                            });
                        }
                        Err(_) => {
                            return Err(tonic::Status::internal("Did not get a stream type back"))
                        }
                    },
                    Err(e) => {
                        // Err(tonic::Status::internal("Could not extract from box"));
                        match e {
                            ExtractorErrors::Err(value) => {
                                return Err(tonic::Status::internal(format!(
                                    "got an error: {}",
                                    value
                                )))
                            }
                            _ => return Err(tonic::Status::internal("got an unknown error")),
                        }
                    }
                }
            }
            Err(e) => match e {
                RouterErrors::NoHandlerFound => {
                    return Err(tonic::Status::internal("Did not get a stream type back"))
                }
            },
        }
        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
    async fn delete(
        &self,
        request: tonic::Request<proto::DeleteServerRequest>,
    ) -> std::result::Result<tonic::Response<proto::DeleteServerResponse>, tonic::Status> {
        let inner = request.into_inner();
        let delete_server_request = DeleteServerRequest {
            common: IncomingMessageWithMetadata {
                message: "delete_server".to_string(),
                message_type: "command".to_string(),
                metadata: inner.clone().metadata.unwrap().into(),
                authcode: "0".to_string(),
            },
        };

        let mut router = self.router.lock().await;
        let response_result = router
            .execute_handler_typed(delete_server_request, "delete_server".to_string())
            .await;
        if let Ok(response) = response_result {
            match response.try_into_response() {
                Ok(boxed) => match boxed.downcast::<String>() {
                    Ok(final_response) => {
                        if let Ok(response) =
                            serde_json::from_str::<proto::DeleteServerResponse>(&*final_response)
                        {
                            Ok(response.into())
                        } else {
                            return Err(tonic::Status::internal("Could not serialize response"));
                        }
                    }
                    Err(_) => Err(tonic::Status::internal(
                        "Response did not come back as a string",
                    )),
                },
                Err(_) => Err(tonic::Status::internal(
                    "Failed during a response conversion",
                )),
            }
        } else {
            Err(tonic::Status::internal(
                "Could not get response back at all",
            ))
        }
    }
    async fn start(
        &self,
        // request: tonic::Request<proto::StartServerRequest>,
        request: tonic::Request<Streaming<proto::ServerMessage>>,
    ) -> std::result::Result<tonic::Response<Self::CreateStream>, tonic::Status> {
        let start_server_request = StartServerRequest::default();

        let (tx, rx) = mpsc::channel(32);

        let inner_router_guard = Arc::clone(&self.router);
        let mut inbound = request.into_inner();
        tokio::spawn(async move {
            while let Some(result) = inbound.next().await {
                let mut router = inner_router_guard.lock().await;
                match result {
                    Ok(message) => {
                        let _ = router
                            .execute_handler_typed(
                                ConsoleRequest {
                                    common: IncomingMessage {
                                        message: message.data,
                                        message_type: "console".to_string(),
                                        authcode: "0".to_string(),
                                    },
                                },
                                "console".to_string(),
                            )
                            .await;
                    }
                    Err(e) => {
                        println!("got an error in the stream");
                    }
                }
            }

            // let mut router = &self.router.lock().await;
            // router
        });
        let mut router = self.router.lock().await;

        let response_result = router
            .execute_handler_typed(start_server_request, "start_server".to_string())
            .await;
        match response_result {
            Ok(response) => {
                match response.try_into_response() {
                    Ok(boxed) => match boxed
                        .downcast::<Pin<Box<dyn Stream<Item = String> + Send + Sync>>>()
                    {
                        Ok(stream_box) => {
                            tokio::spawn(async move {
                                let mut stream = *stream_box;
                                while let Some(message) = stream.next().await {
                                    if let Err(e) = tx
                                        .send(Ok(ServerMessage {
                                            authcode: "0".to_string(),
                                            data: message,
                                            r#type: "console".to_string(),
                                        }))
                                        .await
                                    {
                                        // eprintln!("send failed, dropped: {:?}", e.0);
                                        println!("send failed");
                                    }
                                    //println!("{:#?}", res);
                                }
                                println!("no stream");
                                let _ = tx.send(Err(tonic::Status::aborted("stream EOF"))).await;
                            });
                        }
                        Err(_) => {
                            return Err(tonic::Status::internal("Did not get a stream type back"))
                        }
                    },
                    Err(e) => match e {
                        ExtractorErrors::Err(value) => {
                            return Err(tonic::Status::internal(format!("got an error: {}", value)))
                        }
                        _ => return Err(tonic::Status::internal("got an unknown error")),
                    },
                }
            }
            Err(e) => match e {
                RouterErrors::NoHandlerFound => {
                    return Err(tonic::Status::internal("Did not get a stream type back"))
                }
            },
        }
        println!("returning a stream");
        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
    async fn stop(
        &self,
        _request: tonic::Request<proto::StopServerRequest>,
    ) -> std::result::Result<tonic::Response<proto::StopServerResponse>, tonic::Status> {
        //let inner = request.into_inner();
        let stop_server_request = StopServerRequest::default();

        let mut router = self.router.lock().await;
        let response_result = router
            .execute_handler_typed(stop_server_request, "stop_server".to_string())
            .await;
        if let Ok(response) = response_result {
            match response.try_into_response() {
                Ok(boxed) => match boxed.downcast::<String>() {
                    Ok(final_response) => {
                        if let Ok(response) =
                            serde_json::from_str::<proto::StopServerResponse>(&*final_response)
                        {
                            Ok(response.into())
                        } else {
                            return Err(tonic::Status::internal("Could not serialize response"));
                        }
                    }
                    Err(_) => Err(tonic::Status::internal(
                        "Response did not come back as a string",
                    )),
                },
                Err(_) => Err(tonic::Status::internal(
                    "Failed during a response conversion",
                )),
            }
        } else {
            Err(tonic::Status::internal(
                "Could not get response back at all",
            ))
        }
    }
}

#[tonic::async_trait]
impl ServerManage for Connection {
    async fn data(
        &self,
        _request: tonic::Request<proto::ServerDataRequest>,
    ) -> std::result::Result<tonic::Response<proto::ServerDataResponse>, tonic::Status> {
        let server_data_request = ServerDataRequest::default();

        let mut router = self.router.lock().await;
        let response_result = router
            .execute_handler_typed(server_data_request, "server_data".to_string())
            .await;
        if let Ok(response) = response_result {
            match response.try_into_response() {
                Ok(boxed) => match boxed.downcast::<String>() {
                    Ok(final_response) => {
                        if let Ok(response) =
                            serde_json::from_str::<proto::ServerDataResponse>(&*final_response)
                        {
                            Ok(response.into())
                        } else {
                            return Err(tonic::Status::internal("Could not serialize response"));
                        }
                    }
                    Err(_) => Err(tonic::Status::internal(
                        "Response did not come back as a string",
                    )),
                },
                Err(_) => Err(tonic::Status::internal(
                    "Failed during a response conversion",
                )),
            }
        } else {
            Err(tonic::Status::internal(
                "Could not get response back at all",
            ))
        }
    }

    async fn set(
        &self,
        request: tonic::Request<proto::SetServerRequest>,
    ) -> std::result::Result<tonic::Response<proto::SetServerResponse>, tonic::Status> {
        let server_set_request = SetServerRequest {
            common: IncomingMessageWithMetadata {
                message: "set_server".to_string(),
                message_type: "command".to_string(),
                metadata: request.get_ref().metadata.clone().unwrap().into(),
                authcode: "0".to_string(),
            },
        };

        let mut router = self.router.lock().await;
        let response_result = router
            .execute_handler_typed(server_set_request, "set_server".to_string())
            .await;
        if let Ok(response) = response_result {
            match response.try_into_response() {
                Ok(boxed) => match boxed.downcast::<String>() {
                    Ok(final_response) => {
                        if let Ok(response) =
                            serde_json::from_str::<proto::SetServerResponse>(&*final_response)
                        {
                            Ok(response.into())
                        } else {
                            return Err(tonic::Status::internal("Could not serialize response"));
                        }
                    }
                    Err(_) => Err(tonic::Status::internal(
                        "Response did not come back as a string",
                    )),
                },
                Err(_) => Err(tonic::Status::internal(
                    "Failed during a response conversion",
                )),
            }
        } else {
            Err(tonic::Status::internal(
                "Could not get response back at all",
            ))
        }
    }
    async fn state(
        &self,
        _request: tonic::Request<proto::ServerStateRequest>,
    ) -> std::result::Result<tonic::Response<proto::ServerStateResponse>, tonic::Status> {
        let server_state_request = ServerStateRequest::default();

        let mut router = self.router.lock().await;
        let response_result = router
            .execute_handler_typed(server_state_request, "server_state".to_string())
            .await;
        if let Ok(response) = response_result {
            match response.try_into_response() {
                Ok(boxed) => match boxed.downcast::<String>() {
                    Ok(final_response) => {
                        if let Ok(response) =
                            serde_json::from_str::<proto::ServerStateResponse>(&*final_response)
                        {
                            Ok(response.into())
                        } else {
                            return Err(tonic::Status::internal("Could not serialize response"));
                        }
                    }
                    Err(_) => Err(tonic::Status::internal(
                        "Response did not come back as a string",
                    )),
                },
                Err(_) => Err(tonic::Status::internal(
                    "Failed during a response conversion",
                )),
            }
        } else {
            Err(tonic::Status::internal(
                "Could not get response back at all",
            ))
        }
    }
}

#[tonic::async_trait]
impl NodeManage for Connection {
    async fn name(
        &self,
        request: tonic::Request<proto::ServerNameRequest>,
    ) -> std::result::Result<tonic::Response<proto::ServerNameResponse>, tonic::Status> {
        let server_name_request = ServerNameRequest::default();

        let mut router = self.router.lock().await;
        let response_result = router
            .execute_handler_typed(server_name_request, "server_name".to_string())
            .await;
        if let Ok(response) = response_result {
            match response.try_into_response() {
                Ok(boxed) => match boxed.downcast::<String>() {
                    Ok(final_response) => {
                        if let Ok(response) =
                            serde_json::from_str::<proto::ServerNameResponse>(&*final_response)
                        {
                            Ok(response.into())
                        } else {
                            return Err(tonic::Status::internal("Could not serialize response"));
                        }
                    }
                    Err(_) => Err(tonic::Status::internal(
                        "Response did not come back as a string",
                    )),
                },
                Err(_) => Err(tonic::Status::internal(
                    "Failed during a response conversion",
                )),
            }
        } else {
            Err(tonic::Status::internal(
                "Could not get response back at all",
            ))
        }
    }
}
// impl Into<crate::MetadataTypes> for proto::MetadataTypes {
//     fn into(self) -> crate::MetadataTypes {
//         serde_json::from_value(serde_json::to_value(self).unwrap()).unwrap()
//     }
// }
impl Into<crate::MetadataTypes> for proto::MetadataTypes {
    fn into(self) -> crate::MetadataTypes {
        // TODO: remove hardcoding of server for
        // metadata conversion?
        match self.kind.as_str() {
            "Server" => serde_json::from_str::<crate::MetadataTypes>(&self.data).unwrap(),
            _ => {
                println!("{:#?}", self);
                let value = serde_json::to_value(self.clone()).unwrap();
                println!("{:#?}", value);
                serde_json::from_value(value).unwrap()
            }
        }
    }
}
impl Connection {
    pub async fn serve_with_arc(
        self: Arc<Self>,
        url: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("serving");
        let addr = url.parse()?;
        Server::builder()
            .add_service(ServerEditServer::from_arc(self.clone()))
            .add_service(ServerManageServer::from_arc(self))
            .serve(addr)
            .await?;
        Ok(())
    }
}
pub struct ConnectionHandler {
    // current_request: Option<String>,
    // requests: Vec<String>,
    connection: Connection,
}

impl ConnectionHandler {
    pub fn new(router: Arc<Mutex<Router<Arc<AppState>>>>) -> ConnectionHandler {
        ConnectionHandler {
            // current_request: None,
            // requests: Vec::new(),
            connection: Connection { router },
        }
    }
}

// TODO: work on a macro which leads to GRPC skipping the event loop
// the test the implimentation, find a way to directly connect the GRPC code here
// to the functional code defined in main
//inventory
macro_rules! connection {
    () => {};
}
macro_rules! register_request {
    ($arg1:ty => $arg2:ident) => {};
}
