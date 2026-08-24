use std::{ops::ControlFlow, sync::Arc};

use crate::{AppState, MessagePayload};
use network_abstraction_lib::{FromWire, Router, ValueRequest};
use serde::{Deserialize, Serialize};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::Mutex,
};

use crate::{GetState, IncomingMessage, IncomingMessageWithMetadata, SimpleMessage};

pub struct ConnectionManager {
    listner: TcpListener,
    router: Arc<Mutex<Router<Arc<AppState>>>>,
}
impl ConnectionManager {
    pub async fn serve(
        router: Router<Arc<AppState>>,
        url: String,
    ) -> Result<ConnectionManager, Box<dyn std::error::Error + Send + Sync>> {
        let listner = TcpListener::bind(url).await?;

        Ok(ConnectionManager {
            listner,
            router: Arc::new(Mutex::new(router)),
        })
    }
    pub async fn accept_connection(
        &mut self,
    ) -> Result<(ConnectionHandler, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
        let (socket, addr) = self.listner.accept().await?;
        let handler = ConnectionHandler {
            stream: Some(socket),
            read_buf: vec![],
            newline_pos: 0,
            bytes_filter_method: BytesFilterMethod::Line
        };
        Ok((handler, Some(addr.to_string())))
    }
    pub async fn get_arc_mutex_router(&self) -> Arc<Mutex<Router<Arc<AppState>>>> {
        self.router.clone()
    }
}

static FILE_STARTING_DELIMITER: &str = "\\\\f";

enum BytesFilterMethod {
    Line, 
    All
}

enum Protocol {
    Json(usize),
    FileTransfer(usize),
    Continue(usize)
}

pub struct ConnectionHandler {
    stream: Option<TcpStream>,
    read_buf: Vec<u8>,
    newline_pos: usize,
    bytes_filter_method: BytesFilterMethod
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
        self.remove_segment_or_clear(self.newline_pos);
    }
    fn remove_segment_or_clear(&mut self, position: usize) {
        if position + 1 <= self.inner().len() {
            self.inner().drain(..position + 1);
        } else {
            self.inner().clear();
        }
    }
    pub async fn next(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.read_buf.len() == 0 {
            return Err("empty buffer".into())
        };

        if matches!(self.bytes_filter_method, BytesFilterMethod::Line) {
            let position = self.read_buf
                .windows(FILE_STARTING_DELIMITER.len())
                .enumerate()
                .try_fold(0usize, |_acc, (i, bytes)| {
                    if bytes == FILE_STARTING_DELIMITER.as_bytes() {
                        ControlFlow::Break(Protocol::FileTransfer(i))
                    } else if let Some(pos) = bytes.iter().position(|b| *b == b'\n') {
                        ControlFlow::Break(Protocol::Json(i + pos))
                    } else {
                        ControlFlow::Continue(i + 1)
                    }
                });
            if let ControlFlow::Break(protocol) = position {
                match protocol {
                    Protocol::Json(pos) => {
                        self.newline_pos = pos;
                    },
                    Protocol::FileTransfer(pos) => {
                        self.bytes_filter_method = BytesFilterMethod::All;
                        self.newline_pos = pos;
                    },
                    Protocol::Continue(_) => {
                        println!("continuing");
                    },
                }

                Ok(())
            } else {
                Err("Did not find next position".into())
            }
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
        if matches!(self.bytes_filter_method, BytesFilterMethod::All){
            return Err("Cannot receive line when receiving all bytes".into());
        }

        let newline_pos = self.newline_pos.clone();
        let line = &self.read_buf[..newline_pos];

        if line.is_empty() {
            self.remove_current_segment_or_clear().await;
            return Err("Line is empty".into());
        }

        let line_str = String::from_utf8_lossy(line);
        Ok(line_str.to_string())
    }
    pub async fn append_bytes(&mut self, bytes: Vec<u8>) {
        self.inner().extend_from_slice(&bytes);
    }
    pub async fn has_remaining_buffer(&self) -> bool {
        self.newline_pos + 1 <= self.read_buf.len()
    }

    pub fn split(&mut self) -> Result<(Writer, Reader), Box<dyn std::error::Error + Send + Sync>> {
        let stream = self.stream.take().ok_or("no stream set")?;
        let (read_half, write_half) = stream.into_split();
        Ok((Writer { write_half }, Reader { read_half }))
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
    //read_buf: Option<&Vec<u8>>,
}
impl Reader {
    // TODO: consider removing this or keeping it
    // pub async fn recv(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    //     let mut temp_buf = vec![0u8; 4096];
    //     let n = self.read_half.read(&mut temp_buf).await?;

    //     if n == 0 {
    //         return Err("connection closed by peer or no bytes".into());
    //     }

    //     println!("got {}", String::from_utf8_lossy(&temp_buf[..n]));
    //     Ok(temp_buf)
    // }
    pub async fn handle_request(
        &mut self,
        handler: &mut ConnectionHandler,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut temp_buf = vec![0u8; 4096];
        let n = {
            match self.read_half.read(&mut temp_buf).await {
                Ok(n) => n,
                Err(_) => {
                    return Err("failed to read".into());
                }
            }
        };
        if n == 0 {
            return Err("connection closed by peer or no bytes".into());
        }
        handler.append_bytes(temp_buf[..n].to_vec()).await;
        Ok(())
    }
}
