use std::any::Any;
use std::sync::Arc;
use general_networked_filesystem::{
    chain::ChainBuilder, flume_delimited::{FlumeFile, TcpFsBidirectional, TcpFsReceiver, TcpFsSender}, Codec, Direction, EofFrame, FileFrame, FileOperations, LocalState, Operation, RemoteFileSystem, StreamableFileSystemErrors
};
use tokio::sync::{mpsc, watch, Notify};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
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
        let full_remote_fs = FileSystemHandler::create_file_handler(fs_tx, fs_rx);
        FileSystemHandler {
            arc_file_tx: Arc::new(Mutex::new(full_remote_fs)),
            operations: FileOperations::new(),
        }
    }
    fn create_file_handler(
        fs_tx: flume::Sender<Vec<u8>>,
        fs_rx: flume::Receiver<Vec<u8>>,
    ) -> RemoteFileSystem<TcpFsBidirectional, FlumeFile> {
        let mut file_tx = TcpFsBidirectional::new(fs_rx, fs_tx);
        file_tx.set_start_delimiter(r"\\\\f".as_bytes().to_vec());
        file_tx.set_end_delimiter("////f".as_bytes().to_vec());
        let mut full_remote_fs = RemoteFileSystem::new(file_tx);
        full_remote_fs.set_direction(Direction::Server);
        full_remote_fs
    }
    // pub async fn create_basic_file_stream(&self, mut raw_rx: mpsc::UnboundedReceiver<Vec<u8>>) -> mpsc::UnboundedReceiver<Vec<u8>> {
    pub async fn create_basic_file_stream(mut raw_rx: flume::Receiver<Vec<u8>>, end_file_task: Arc<CancellationToken>) -> flume::Receiver<Vec<u8>> {
        let (fs_out_tx, fs_out_rx) = flume::unbounded();

        tokio::spawn(async move {
            let (fs_in_tx, fs_in_rx) = flume::unbounded();
            let mut handler = FileSystemHandler::create_file_handler(fs_in_tx.clone(), fs_in_rx.clone());
            let mut chain_builder = ChainBuilder::new(&mut handler);
            let mut chain = chain_builder
                .chain::<FileFrame, _, _>(move |_, mut f, fs| {
                    let inner_fs_out_tx = fs_out_tx.clone();
                    Box::pin(async move {
                        let res = inner_fs_out_tx.send_async(f.chunks).await;
                        Ok(())
                    })
                });
            let inner_end_file_task = end_file_task.clone();
            let mut chain = chain
                .chain::<EofFrame, _, _>(move |_, mut e, fs| {
                    let inner_end_file_task = inner_end_file_task.clone();
                    Box::pin(async move {
                        inner_end_file_task.cancel();
                        Ok(())
                    })
                });
            loop {
                tokio::select! {
                    Ok(bytes) = raw_rx.recv_async() => {
                        let remainder = &mut 0;
                        tokio::select! {
                            _ = chain.decode_bytes(0, bytes, remainder) => {},
                            _ = end_file_task.cancelled() => {
                                break;
                            }
                        }
                    }
                    _ = end_file_task.cancelled() => {
                        break;
                    }
                }
            }
        });
        fs_out_rx
    }

    pub async fn download(&self, location: String){
        self.set_location(location).await;
        let mut file_tx = self.arc_file_tx.lock().await.clone();
        let _ = file_tx.set_operation(Operation::Drain);

        let _ = RemoteFileSystem::execute_operation_bidirectionally(&mut file_tx, 0).await;
    }
    pub async fn set_location(&self, location: String){
        let mut file_tx = self.arc_file_tx.lock().await.clone();
        let _ = file_tx.set_location(0, location).await;
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
