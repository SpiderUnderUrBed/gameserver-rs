use std::any::Any;

use general_networked_filesystem::{flume_delimited::{FlumeFile, TcpFsReceiver, TcpFsSender}, Codec, Direction, FileOperations, LocalState, Operation, RemoteFileSystem, StreamableFileSystemErrors};

#[derive(Debug)]
pub enum FilesystemErrors {
    Any(Box<dyn Any + Sync + Send>)

}

#[allow(unused)]
pub struct FileSystemHandler {
    pub file_tx: RemoteFileSystem<TcpFsSender, FlumeFile>,
    file_rx: RemoteFileSystem<TcpFsReceiver, FlumeFile>,
    operations: FileOperations,
}
impl FileSystemHandler {
    pub fn new(fs_sender_tx: flume::Sender<Vec<u8>>, fs_sender_rx: flume::Receiver<Vec<u8>>, fs_receiver_tx: flume::Sender<Vec<u8>>, fs_receiver_rx: flume::Receiver<Vec<u8>>) -> FileSystemHandler {
        let mut fs_sender = TcpFsSender::new(fs_sender_rx, fs_sender_tx);
        let mut fs_receiver = TcpFsReceiver::new(fs_receiver_tx, fs_receiver_rx);
        fs_sender.set_start_delimiter(r"\\f".as_bytes().to_vec());
        fs_sender.set_end_delimiter("//f".as_bytes().to_vec());
        fs_receiver.set_start_delimiter(r"\\f".as_bytes().to_vec());
        fs_receiver.set_end_delimiter("//f".as_bytes().to_vec());
        FileSystemHandler {
            file_tx: RemoteFileSystem::new(fs_sender),
            file_rx: RemoteFileSystem::new(fs_receiver),
            operations: FileOperations::new(),
        }
    }
    pub fn send_flume_file(&mut self, original_location: Option<String>, final_location: String, content_stream: Option<flume::Receiver<Vec<u8>>>){
        // let filesystem_sender: &mut RemoteFileSystem<TcpFsSender, FlumeFile> =
        //     &mut self.file_tx;
        let file = FlumeFile {
            original_location,
            final_location,
            content_stream,
        };
        self.file_tx.set_codec(Codec::RawContinues);
        self.file_tx.set_direction(Direction::Server);
        let _ = self.file_tx.set_operation(Operation::Move);
        self.file_tx.append_files(file);
    }
    pub async fn execute_operation(&mut self, state_id: u8) -> Result<(), StreamableFileSystemErrors> {
        // self.file_tx.execute_operation(state_id).await
        //     .map_err(|e| FilesystemErrors::Any(Box::new(e)))
        self.file_tx.execute_operation(state_id).await
    }
    pub async fn proxy_receiver(&mut self) -> flume::Receiver<Vec<u8>>{
        self.file_tx.inner_mut().rx.clone()
    }
    pub fn create_state(&mut self, state_id: u8, location: String){
        self.file_tx.create_state(state_id, LocalState {
            location,
        });
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