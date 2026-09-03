use std::{ops::ControlFlow, sync::Arc};

use crate::{AppState, MessagePayload};
use general_networked_filesystem::{chain::ChainBuilder, FileFrame, FileHandleStatus, LocalState, SetFrame};
use general_networked_filesystem::{EofFrame, FrameCommons};
use network_abstraction_lib::{FromWire, Router, ValueRequest};
use serde::{Deserialize, Serialize};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::{watch, Mutex},
};

use crate::{GetState, IncomingMessage, IncomingMessageWithMetadata, SimpleMessage};


#[derive(Debug)]
pub enum BackgroundTaskUpdates {
    None,
    NoMoreFileTransfer
}
// struct Test {

// }
pub async fn spawn_conn_background_tasks(arc_state: Arc<AppState>, arc_conn_manager: Arc<Mutex<ConnectionManager>>) {
    let (watch_tx, watch_rx) = watch::channel(BackgroundTaskUpdates::None);
    let mut conn_manager = arc_conn_manager.lock().await;
    conn_manager.backround_task_updates = Some(Arc::new(watch_rx));
    drop(conn_manager);

    tokio::spawn(async move {
        
        let filesystem_reader = arc_state.filesystem.write().await;
        let file_rx = &mut filesystem_reader.file_rx.clone();
        file_rx.create_state(0, LocalState {
            location: "server/".to_owned(),
        });
        drop(filesystem_reader);

        let mut chain_builder = ChainBuilder::new(file_rx);

        let arc_location = Arc::new(Mutex::new(String::new()));
        let inner_location = Arc::clone(&arc_location);
        let mut chain = chain_builder
            .chain::<FileFrame, _, _>(move |_, mut f, fs| {
                let inner_location = inner_location.clone();
                Box::pin(async move {
                    let location = inner_location.lock().await;
                    let _ = FileFrame::write_at_location(
                        &mut f,
                        fs,
                        location.to_string(),
                    );
                    Ok(())
                })
            });
        let inner_watch_tx = watch_tx.clone();
        let mut chain = chain.chain::<SetFrame, _, _>(move |_, mut s, fs| {
            Box::pin({
            let inner_location = arc_location.clone();
            let inner_watch_tx = inner_watch_tx.clone();
            async move {
                let _ = inner_watch_tx.send(BackgroundTaskUpdates::None);
                if let Ok(location_from_chunks) = String::from_utf8(s.chunks){
                    *inner_location.lock().await = location_from_chunks;
                    println!("past location set");
                    Ok(())
                } else {
                    Err(FileHandleStatus::IncorrectData)
                }
            }
            })
        });
        let mut chain = chain.chain::<EofFrame, _, _>(move |state_id, eof, fs| {
            Box::pin({
                let inner_watch_tx = watch_tx.clone();
                async move {
                    let _ = inner_watch_tx.send(BackgroundTaskUpdates::NoMoreFileTransfer);
                    let _ = EofFrame::handle::<_, _>(eof, state_id, fs).await;
                    Ok(())
                }
            })
        });

        loop {
            let res = chain.run(0).await;
            println!("got a receive {:#?}", res);

            if res.is_err() {
                break;
            }
        }
    });
    // Arc::new(watch_rx)
}

pub struct ConnectionManager {
    listner: TcpListener,
    router: Arc<Mutex<Router<Arc<AppState>>>>,
    backround_task_updates: Option<Arc<watch::Receiver<BackgroundTaskUpdates>>>
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
            backround_task_updates: None,
        })
    }
    // pub async fn subscribe_to_background_tasks(&mut self, task: Arc<watch::Receiver<BackgroundTaskUpdates>>){
    //     self.backround_task_updates = Some(task);
    // }
    pub async fn accept_connection(
        &mut self,
    ) -> Result<(ConnectionHandler, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
        let (socket, addr) = self.listner.accept().await?;
        let handler = ConnectionHandler {
            stream: Some(socket),
            read_buf: vec![],
            newline_pos: 0,
            bytes_filter_method: BytesFilterMethod::Line,
            backround_task_updates: self.backround_task_updates.clone(),
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
    bytes_filter_method: BytesFilterMethod,
    backround_task_updates: Option<Arc<watch::Receiver<BackgroundTaskUpdates>>>
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
        if let Some(backround_task_updates) = &self.backround_task_updates {
            if backround_task_updates.has_changed()? {
                if matches!(*backround_task_updates.borrow(), BackgroundTaskUpdates::NoMoreFileTransfer){
                    self.bytes_filter_method = BytesFilterMethod::Line;
                }
            }
        }

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
