
use networked_filesystem::StreamableFileSystemErrors;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

pub enum Direction {
    Server,
    Local
}

#[derive(Clone)]
pub struct FileSystemHandler {
    eof_task: Arc<Notify>,
}
impl FileSystemHandler {
    pub fn new(
        _fs_tx: flume::Sender<Vec<u8>>,
        _fs_rx: flume::Receiver<Vec<u8>>,
        _direction: Direction,
    ) -> FileSystemHandler {
        FileSystemHandler {
            eof_task: Arc::new(Notify::new())
        }
    }
    pub async fn create_basic_file_stream(raw_rx: flume::Receiver<Vec<u8>>, _direction: Direction, _end_file_task: Arc<CancellationToken>) -> flume::Receiver<Vec<u8>> {
        raw_rx
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
        _original_location: Option<String>,
        _final_location: String,
        _content_stream: Option<flume::Receiver<Vec<u8>>>,
    ) {
    }
    pub async fn execute_operation(
        &mut self,
        _state_id: u8,
    ) -> Result<(), StreamableFileSystemErrors> {
        Ok(())
    }
    pub async fn create_state(&mut self, _state_id: u8, _location: String) {
    }
    pub async fn proxy_receiver(&mut self) -> flume::Receiver<Vec<u8>> {
        let (_, fs_out_rx) = flume::unbounded();
        fs_out_rx
    }
}