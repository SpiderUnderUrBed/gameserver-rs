use std::any::Any;
use std::sync::Arc;
use general_networked_filesystem::{
    flume_delimited::{FlumeFile, TcpFsBidirectional, TcpFsReceiver, TcpFsSender}, Codec, Direction, FileOperations, LocalState, Operation, RemoteFileSystem, StreamableFileSystemErrors
};
use tokio::sync::watch;
use tokio::sync::Mutex;
#[derive(Debug)]
pub enum FilesystemErrors {
    Any(Box<dyn Any + Sync + Send>),
}

#[allow(unused)]
#[derive(Clone)]
pub struct FileSystemHandler {
    pub arc_file_tx: Arc<Mutex<RemoteFileSystem<TcpFsBidirectional, FlumeFile>>>,
    operations: FileOperations,
}
impl FileSystemHandler {
    pub fn new(
        fs_tx: flume::Sender<Vec<u8>>,
        fs_rx: flume::Receiver<Vec<u8>>,
        // fs_receiver_tx: flume::Sender<Vec<u8>>,
        // fs_receiver_rx: flume::Receiver<Vec<u8>>,
    ) -> FileSystemHandler {
        let mut file_tx = TcpFsBidirectional::new(fs_rx, fs_tx);
        file_tx.set_start_delimiter(r"\\\\f".as_bytes().to_vec());
        file_tx.set_end_delimiter("////f".as_bytes().to_vec());
        let mut full_remote_fs = RemoteFileSystem::new(file_tx);
        full_remote_fs.set_direction(Direction::Local);
        FileSystemHandler {
            arc_file_tx: Arc::new(Mutex::new(full_remote_fs)),
            operations: FileOperations::new(),
        }
    }
    pub async fn upload(&self){
        //arc_file_tx: Arc<Mutex<self>>
        // self.file_tx.set_codec(Codec::RawContinues);
        // self.file_tx.set_direction(Direction::Server);
        let mut file_tx = self.arc_file_tx.lock().await;
        let _ = file_tx.set_operation(Operation::Drain);
        drop(file_tx);
        // tokio::spawn(async move {
        RemoteFileSystem::create_bidirectional_handler(self.arc_file_tx.clone(), 0, None).await;
        //});
        // let mut file_tx = self.arc_file_tx.lock().await;
        // let _ = file_tx.set_operation(Operation::Drain);
        // file_tx.execute_operation(0).await;
        // drop(file_tx);
    }
    pub async fn send_flume_file(
        &mut self,
        original_location: Option<String>,
        content_stream: Option<flume::Receiver<Vec<u8>>>,
    ) -> Result<(), StreamableFileSystemErrors> {
        let mut file_tx = self.arc_file_tx.lock().await;
        let file = FlumeFile {
            original_location,
            final_location: String::new(),
            content_stream,
        };
        file_tx.set_codec(Codec::RawContinues);
        file_tx.set_direction(Direction::Server);
        let _ = file_tx.set_operation(Operation::Move);
        file_tx.send_file(file).await?;
        Ok(())
    }
    pub async fn add_flume_file(
        &mut self,
        original_location: Option<String>,
        final_location: String,
        content_stream: Option<flume::Receiver<Vec<u8>>>,
    ) {
        let mut file_tx = self.arc_file_tx.lock().await;
        let file = FlumeFile {
            original_location,
            final_location,
            content_stream,
        };
        file_tx.set_codec(Codec::RawContinues);
        file_tx.set_direction(Direction::Server);
        let _ = file_tx.set_operation(Operation::Move);
        file_tx.append_files(file);
    }
    pub async fn execute_operation(
        &mut self,
        state_id: u8,
    ) -> Result<(), StreamableFileSystemErrors> {
        // self.file_tx.execute_operation(state_id).await
        //     .map_err(|e| FilesystemErrors::Any(Box::new(e)))
        let file_tx = self.arc_file_tx.lock().await;
        file_tx.clone().execute_operation(state_id).await
    }
    pub async fn proxy_receiver(&mut self) -> flume::Receiver<Vec<u8>> {
        let mut file_tx = self.arc_file_tx.lock().await;
        file_tx.inner_mut().rx.clone()
    }
    pub async fn create_state(&mut self, state_id: u8, location: String) {
        let mut file_tx = self.arc_file_tx.lock().await;
        file_tx.create_state(state_id, LocalState { location });
    }
    pub async fn get_operation_event(&self) -> watch::Receiver<Operation> {
        let file_tx = self.arc_file_tx.lock().await;
        file_tx.get_operation_event()
    }
    // pub fn set_start_delimiter(&mut self, start_delimiter: Vec<u8>){
    //     let tx = self.file_tx.inner_mut();
    //     let rx = self.file_rx.inner_mut();
    //     tx.set_start_delimiter(start_delimiter.clone());
    //     rx.set_start_delimiter(start_delimiter);
    // }
    // pub fn set_end_delimiter(&mut self, end_delimiter: Vec<u8>){
    //     let tx = self.file_tx.inner_mut();
    //     let rx = self.file_rx.inner_mut();
    //     tx.set_end_delimiter(end_delimiter.clone());
    //     rx.set_end_delimiter(end_delimiter);
    // }
    // pub fn set_escape_byte(&mut self, escape_byte: u8){
    //     let tx = self.file_tx.inner_mut();
    //     let rx = self.file_rx.inner_mut();
    //     tx.set_escape_byte(escape_byte.clone());
    //     rx.set_escape_byte(escape_byte);
    // }
}
