
use networked_filesystem::StreamableFileSystemErrors;
use std::{any::Any, sync::Arc};
use tokio::sync::Notify;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub enum FilesystemErrors {
    Any(Box<dyn Any + Sync + Send>),
    Unauthorized
}


pub enum Direction {
    Server,
    Local
}

#[derive(Clone)]
pub struct FileSystemHandler {
    eof_task: Arc<Notify>,
    sandboxed_location: Option<String>,
    output_channel: (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>),
    current_file_stream: Arc<Mutex<Option<flume::Receiver<Vec<u8>>>>>
}
impl FileSystemHandler {
    pub fn new(
        _fs_tx: flume::Sender<Vec<u8>>,
        _fs_rx: flume::Receiver<Vec<u8>>,
        _direction: Direction,
    ) -> FileSystemHandler {
        let (fs_tx, fs_rx) = flume::unbounded();
        FileSystemHandler {
            eof_task: Arc::new(Notify::new()),
            sandboxed_location: None,
            current_file_stream: Arc::new(Mutex::new(None)),
            output_channel: (fs_tx, fs_rx),
        }
    }
    pub fn set_sandboxed_location(&mut self, location: String) {
        self.sandboxed_location = Some(location);
    }
   // TODO: in the future run cannonolize on the remote system
    // or do some sort of path validation internally
    // consider a watch channel with location and a rx channel which 
    // checks for a path confirmation from the server after cannonolizing
    pub async fn try_create_request_in_directory(&self, location: String) -> Result<String, FilesystemErrors>{
        if let Some(sandboxed_location) = &self.sandboxed_location { 
            Ok(format!("{}{}", sandboxed_location, location))
        } else {
            Ok(location)
        }
    }
    pub async fn create_basic_file_stream(raw_rx: flume::Receiver<Vec<u8>>, _direction: Direction, _end_file_task: Arc<CancellationToken>) -> flume::Receiver<Vec<u8>> {
        raw_rx
    }
    pub async fn upload(
        &mut self,
        _state_id: u8,
    ) -> Result<(), StreamableFileSystemErrors> {
        let out_tx = self.output_channel.0.clone();
        let arc_current_file_receiver = self.current_file_stream.clone();
        tokio::spawn(async move {
            let mut current_file_receiver_option = arc_current_file_receiver.lock().await;
            if let Some(current_file_receiver) = current_file_receiver_option.take() {
                while let Ok(bytes) = current_file_receiver.recv_async().await {
                    let _ = out_tx.send(bytes);
                }
            }
        });
        Ok(())
    }
    pub async fn download(&self, _location: String){
    }
    pub async fn call_eof(&self){
        self.eof_task.notify_one();
    }
    pub async fn wait_for_eof(&self) {
        self.eof_task.notified().await
    }
    pub async fn set_location(&self, _location: String){
    }
    pub async fn send_flume_file(
        &mut self,
        _original_location: Option<String>,
        _content_stream: Option<flume::Receiver<Vec<u8>>>,
    ) -> Result<(), StreamableFileSystemErrors> {
        Ok(())
    }
    pub async fn add_flume_file(
        &mut self,
        _state_id: u8,
        _original_location: Option<String>,
        _final_location: String,
        content_stream: Option<flume::Receiver<Vec<u8>>>,
    ) {
        *self.current_file_stream.lock().await = content_stream;
    }
    pub async fn execute_operation(
        &mut self,
        state_id: u8,
    ) -> Result<(), StreamableFileSystemErrors> {
        // let (fs_out_tx, fs_out_rx) = flume::unbounded();
        self.upload(state_id).await
    }
    pub async fn create_state(&mut self, _state_id: u8, _location: String) {
    }
    pub async fn proxy_receiver(&mut self) -> flume::Receiver<Vec<u8>> {
        self.output_channel.1.clone()
    }
}