use crate::extra::JsonAssembler;
use crate::MessagePayload;
use crate::Sender;
use crate::{extra, IncomingMessage};
use async_trait::async_trait;
use multer::Multipart;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::fmt;
use std::{
    any::Any,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::fs;
use tokio::fs::File;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::time::timeout;

use tokio::sync::mpsc;

use std::io::SeekFrom;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
pub enum FsItem {
    File(String),
    Folder(String),
}

#[async_trait::async_trait]
pub trait FsType: Clone + Send + Sync {
    async fn get_files_content(
        &mut self,
        file_chunk: FileChunk,
    ) -> std::io::Result<IncomingMessage>;
    async fn get_metadata(&mut self, path: &str) -> std::io::Result<FsMetadata>;
    async fn list_directory(&mut self, path: &str) -> std::io::Result<Vec<FsEntry>>;
    async fn list_directory_within_range(
        &mut self,
        path: &str,
        start: Option<u64>,
        end: Option<u64>,
    ) -> std::io::Result<Vec<FsEntry>>;
    async fn get_path_from_tag(&mut self, tag: &str) -> std::io::Result<Vec<String>>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsMetadata {
    pub is_file: bool,
    pub is_dir: bool,
    pub optional_folder_children: Option<u64>,
    pub canonical_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsEntry {
    pub name: String,
    pub is_file: bool,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BasicPath {
    pub paths: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct FileRequestMessage {
    id: u64,
    #[serde(flatten)]
    payload: FileRequestPayload,
}

// FileChunk represents a portion of a file, with file name
// ensuring its written to the write file, offsets and size to make sure it doesnt already cover content written to a file
// and size for defining when the size of the buffer, to ensure nothing unexpected happens (might be used to help determine when the chunk is done)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FileChunk {
    pub(crate) file_name: String,
    pub(crate) file_chunk_offet: String,
    pub(crate) file_chunk_size: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum FileRequestPayload {
    Metadata {
        path: String,
    },
    ListDir {
        path: String,
    },
    ListDirWithRange {
        path: String,
        start: Option<u64>,
        end: Option<u64>,
    },
    PathFromTag {
        path: String,
        tag: Option<String>,
    },
    FileChunk(FileChunk),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileResponseMessage {
    pub in_response_to: u64,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OneTimeWrapper {
    pub data: Value,
}

#[derive(Debug)]
pub struct TcpFs {
    pub tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    pub tcp_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    pub request_id: Arc<AtomicU64>,
}

impl Clone for TcpFs {
    fn clone(&self) -> Self {
        let cloned = TcpFs {
            tcp_tx: self.tcp_tx.clone(),
            tcp_rx: self.tcp_tx.subscribe(),
            request_id: self.request_id.clone(),
        };
        cloned
    }
}

use tokio::sync::mpsc::Sender as MpscSender;
use tokio::time::sleep;

impl TcpFs {
    pub fn new(
        tx: tokio::sync::broadcast::Sender<Vec<u8>>,
        rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            tcp_tx: tx,
            tcp_rx: rx,
            request_id: Arc::new(AtomicU64::new(0)),
        }
    }

    async fn send_request(&mut self, payload: FileRequestPayload) -> std::io::Result<u64> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = FileRequestMessage { id, payload };

        let json_string = serde_json::to_string(&request)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let serialized = (json_string + "\n").into_bytes();
        self.tcp_tx
            .send(serialized)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionAborted, e))?;
        Ok(id)
    }

    pub async fn recv_response(&mut self, id: u64) -> std::io::Result<Vec<Vec<u8>>> {
        use tokio::time::{Duration, Instant};

        let timeout_duration = Duration::from_secs(10);
        let start_time = Instant::now();
        let mut assembler = JsonAssembler::new();
        let mut expecting_fragments = false;

        loop {
            if start_time.elapsed() > timeout_duration {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Timeout waiting for response to request {}", id),
                ));
            }

            let response = self
                .tcp_rx
                .recv()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionAborted, e))?;

            if response.is_empty() {
                continue;
            }

            let response_str = String::from_utf8_lossy(&response);

            if expecting_fragments {
            } else {
                if response_str.contains("\"type\":\"Metadata\"")
                    || response_str.contains("\"type\":\"ListDir\"")
                    || response_str.contains("\"type\":\"FileChunk\"")
                {
                    continue;
                }

                if !response_str.contains("\"in_response_to\"") {
                    continue;
                }

                if !response_str.contains(&format!("\"in_response_to\":{}", id)) {
                    continue;
                }

                if response_str.starts_with("{\"in_response_to\"") {
                    expecting_fragments = true;
                }
            }

            let preview_snippet: &str = if response_str.len() > 512 {
                &response_str[..512]
            } else {
                &response_str
            };
            let before_len = assembler.buffer.len();

            let completed = assembler.feed_chunk(&response_str, id).await;

            let after_len = assembler.buffer.len();

            if !completed.is_empty() {
                expecting_fragments = false;
                return Ok(completed);
            }

            if expecting_fragments && assembler.buffer.len() > 0 {
                continue;
            }

            if let Some(result) = assembler.check_timeout(id) {
                match &result {
                    Ok(bytes) => {
                        let s = String::from_utf8_lossy(bytes);
                    }
                    Err(e) => {
                        println!(
                            "[recv_response:{}] assembler.check_timeout error: {}",
                            id, e
                        );
                    }
                }
                expecting_fragments = false;
                return result.map(|v| vec![v]);
            }
        }
    }
}

use walkdir::WalkDir;

const ENABLE_BROADCAST_LOGS: bool = true;

pub async fn send_folder_over_broadcast<P: AsRef<Path>>(
    folder: P,
    writer_tx: mpsc::Sender<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    const FILE_DELIMITER: &[u8] = b"<|END_OF_FILE|>";

    let folder_path = folder.as_ref();

    if !folder_path.exists() {
        return Err(format!("Folder does not exist: {:?}", folder_path).into());
    }

    let mut entries = Vec::new();
    collect_files(folder_path, folder_path, &mut entries)?;

    println!("Starting transfer of {} files", entries.len());

    for (i, (relative_path, full_path)) in entries.iter().enumerate() {
        let expected_size = match tokio::fs::metadata(&full_path).await {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                eprintln!("Failed to get metadata for {}: {}", relative_path, e);
                continue;
            }
        };

        println!(
            "Sending file {}/{}: {} ({} bytes expected)",
            i + 1,
            entries.len(),
            relative_path,
            expected_size
        );

        let start_msg = serde_json::json!({
            "type": "start_file",
            "message": relative_path,
            "authcode": "0"
        });
        let start_json = format!("{}\n", start_msg);

        println!("Sending start_file: {}", start_json.trim());

        if let Err(e) = writer_tx.send(start_json.into_bytes()).await {
            eprintln!(
                "CRITICAL: Failed to send start_file for {}: {}",
                relative_path, e
            );
            return Err(format!("Failed to send start_file: {}", e).into());
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        if expected_size > 0 {
            match tokio::fs::File::open(&full_path).await {
                Ok(mut file) => {
                    let mut buffer = vec![0u8; 4096];
                    let mut total_bytes_sent = 0u64;
                    let mut chunk_count = 0u64;

                    loop {
                        match file.read(&mut buffer).await {
                            Ok(0) => {
                                println!(
                                    "EOF reached for {}: {} bytes sent in {} chunks",
                                    relative_path, total_bytes_sent, chunk_count
                                );
                                break;
                            }
                            Ok(bytes_read) => {
                                chunk_count += 1;
                                let chunk_data = buffer[..bytes_read].to_vec();

                                match writer_tx.send(chunk_data).await {
                                    Ok(()) => {
                                        total_bytes_sent += bytes_read as u64;

                                        if total_bytes_sent % (5 * 1024 * 1024) == 0
                                            || (total_bytes_sent > 0
                                                && total_bytes_sent % 10_000_000 == 0)
                                        {
                                            println!(
                                                "  Progress: {} / {} bytes ({:.1}%) for {}",
                                                total_bytes_sent,
                                                expected_size,
                                                (total_bytes_sent as f64 / expected_size as f64)
                                                    * 100.0,
                                                relative_path
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "CRITICAL: Failed to send chunk {} for {}: {}",
                                            chunk_count, relative_path, e
                                        );
                                        return Err(format!("Failed to send chunk: {}", e).into());
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to read from {}: {}", relative_path, e);
                                return Err(format!("Failed to read file: {}", e).into());
                            }
                        }
                    }

                    if total_bytes_sent != expected_size {
                        eprintln!(
                            "WARNING: Size mismatch for {}! Expected: {}, Sent: {}",
                            relative_path, expected_size, total_bytes_sent
                        );
                    } else {
                        println!(
                            "Successfully sent all {} bytes for: {}",
                            total_bytes_sent, relative_path
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Failed to open file {}: {}", relative_path, e);
                    continue;
                }
            }
        } else {
            println!("File is empty, skipping data transfer");
        }

        let end_msg = serde_json::json!({
            "type": "end_file",
            "message": relative_path,
            "authcode": "0"
        });
        let end_json = format!("{}\n", end_msg);

        println!("Sending end_file: {}", end_json.trim());

        if let Err(e) = writer_tx.send(end_json.into_bytes()).await {
            eprintln!(
                "CRITICAL: Failed to send end_file for {}: {}",
                relative_path, e
            );
            return Err(format!("Failed to send end_file: {}", e).into());
        }

        if let Err(e) = writer_tx.send(FILE_DELIMITER.to_vec()).await {
            eprintln!("Failed to send delimiter: {}", e);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        println!(
            "Completed file: {} ({} bytes)",
            relative_path, expected_size
        );
    }

    println!(
        "Successfully completed transfer of all {} files",
        entries.len()
    );
    Ok(())
}

pub async fn send_multipart_over_broadcast(
    mut multipart: Multipart<'_>,
    tx: broadcast::Sender<Vec<u8>>,
) -> std::io::Result<()> {
    const FILE_DELIMITER: &[u8] = b"<|END_OF_FILE|>";

    let mut processed_files = Vec::new();

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
    {
        let file_name = field.file_name().unwrap_or("file.bin").to_string();
        processed_files.push(file_name.clone());

        let start_json = MessagePayload {
            r#type: "start_file".into(),
            message: file_name.clone(),
            authcode: "0".into(),
        };
        tx.send(serde_json::to_vec(&start_json)?)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "broadcast send failed"))?;

        let mut sent_chunks = false;

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        {
            tx.send(chunk.to_vec()).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "broadcast send failed")
            })?;

            sent_chunks = true;
        }

        if sent_chunks {
            tx.send(FILE_DELIMITER.to_vec()).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "broadcast send failed")
            })?;
        }

        let end_json = MessagePayload {
            r#type: "end_file".into(),
            message: file_name.clone(),
            authcode: "0".into(),
        };
        tx.send(serde_json::to_vec(&end_json)?)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "broadcast send failed"))?;
    }

    for file_name in processed_files {
        tx.send(FILE_DELIMITER.to_vec())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "broadcast send failed"))?;

        let clean_json = MessagePayload {
            r#type: "clean_file".into(),
            message: file_name,
            authcode: "0".into(),
        };
        tx.send(serde_json::to_vec(&clean_json)?)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "broadcast send failed"))?;
    }

    Ok(())
}

pub async fn cleanup_end_file_markers(file_path: &str, file_name: &str) -> std::io::Result<()> {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let mut file = tokio::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(file_path)
        .await?;

    let mut content = Vec::new();
    file.read_to_end(&mut content).await?;

    let mut content_str = String::from_utf8_lossy(&content).to_string();

    content_str = content_str.trim_end().to_string();

    if let Some(last_brace_pos) = content_str.rfind('}') {
        let slice_up_to_last = &content_str[..=last_brace_pos];
        if let Some(open_brace_pos) = slice_up_to_last.rfind('{') {
            let candidate_json = &slice_up_to_last[open_brace_pos..=last_brace_pos];

            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(candidate_json) {
                if json_value.get("type")
                    == Some(&serde_json::Value::String("end_file".to_string()))
                    && json_value.get("message")
                        == Some(&serde_json::Value::String(file_name.to_string()))
                {
                    let truncate_len = open_brace_pos;
                    file.set_len(truncate_len as u64).await?;
                    file.sync_all().await?;
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

fn find_last_json_end(data: &[u8]) -> Option<usize> {
    data.iter().rposition(|&b| b == b'}')
}

fn find_json_start_before(data: &[u8], end_pos: usize) -> Option<usize> {
    let mut brace_count = 1;
    let mut pos = end_pos;

    while pos > 0 {
        pos -= 1;
        match data[pos] {
            b'}' => brace_count += 1,
            b'{' => {
                brace_count -= 1;
                if brace_count == 0 {
                    return Some(pos);
                }
            }
            _ => {}
        }
    }

    None
}

fn is_end_file_message(json_value: &Value, expected_filename: &str) -> bool {
    if let (Some(msg_type), Some(message)) = (
        json_value.get("type").and_then(|v| v.as_str()),
        json_value.get("message").and_then(|v| v.as_str()),
    ) {
        msg_type == "end_file" && message == expected_filename
    } else {
        false
    }
}

fn collect_files(
    current_dir: &Path,
    base_dir: &Path,
    entries: &mut Vec<(String, PathBuf)>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_files(&path, base_dir, entries)?;
        } else if path.is_file() {
            let relative_path = path
                .strip_prefix(base_dir)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                .to_string_lossy()
                .to_string();
            entries.push((relative_path, path));
        }
    }

    Ok(())
}

fn extract_entries_from_value(val: &Value) -> std::io::Result<Option<Vec<FsEntry>>> {
    if val.is_array() {
        if let Ok(entries) = serde_json::from_value::<Vec<FsEntry>>(val.clone()) {
            return Ok(Some(entries));
        }
        if let Some(arr) = val.as_array() {
            let mut out = Vec::new();
            for item in arr {
                if let Some(name) = item.as_str() {
                    out.push(FsEntry {
                        name: name.to_string(),
                        is_file: true,
                        is_dir: false,
                    });
                } else {
                    return Ok(None);
                }
            }
            return Ok(Some(out));
        }
    }

    if let Some(data_val) = val.get("data") {
        if data_val.is_array() || data_val.is_object() {
            if let Ok(entries) = serde_json::from_value::<Vec<FsEntry>>(data_val.clone()) {
                return Ok(Some(entries));
            }
            if let Some(arr) = data_val.as_array() {
                let mut out = Vec::new();
                for item in arr {
                    if let Some(name) = item.as_str() {
                        out.push(FsEntry {
                            name: name.to_string(),
                            is_file: true,
                            is_dir: false,
                        });
                    } else {
                        return Ok(None);
                    }
                }
                return Ok(Some(out));
            }
        }

        if let Some(s) = data_val.as_str() {
            if let Ok(entries) = serde_json::from_str::<Vec<FsEntry>>(s) {
                return Ok(Some(entries));
            }
            if let Ok(val2) = serde_json::from_str::<Value>(s) {
                if let Some(arr) = val2.as_array() {
                    let mut out = Vec::new();
                    for item in arr {
                        if let Some(name) = item.as_str() {
                            out.push(FsEntry {
                                name: name.to_string(),
                                is_file: true,
                                is_dir: false,
                            });
                        } else {
                            return Ok(None);
                        }
                    }
                    return Ok(Some(out));
                }
            }
        }
    }

    if let Some(list_val) = val.get("list") {
        if list_val.is_array() {
            if let Ok(entries) = serde_json::from_value::<Vec<FsEntry>>(list_val.clone()) {
                return Ok(Some(entries));
            }
            if let Some(arr) = list_val.as_array() {
                let mut out = Vec::new();
                for item in arr {
                    if let Some(name) = item.as_str() {
                        out.push(FsEntry {
                            name: name.to_string(),
                            is_file: true,
                            is_dir: false,
                        });
                    } else {
                        return Ok(None);
                    }
                }
                return Ok(Some(out));
            }
        }
    }

    Ok(None)
}

fn parse_directory_response(response_chunks: &[Vec<u8>], id: u64) -> std::io::Result<Vec<FsEntry>> {
    for chunk in response_chunks.iter() {
        if let Ok(entries) = serde_json::from_slice::<Vec<FsEntry>>(chunk) {
            return Ok(entries);
        }

        if let Ok(val) = serde_json::from_slice::<Value>(chunk) {
            if let Some(entries) = extract_entries_from_value(&val)? {
                return Ok(entries);
            }
        }

        if let Ok(as_str) = std::str::from_utf8(chunk) {
            let trimmed = as_str.trim();
            if trimmed.contains('\n') {
                let out: Vec<FsEntry> = trimmed
                    .lines()
                    .map(|l| FsEntry {
                        name: l.trim().to_string(),
                        is_file: true,
                        is_dir: false,
                    })
                    .collect();
                if !out.is_empty() {
                    return Ok(out);
                }
            }
        }
    }

    let response: Vec<u8> = response_chunks.concat();
    let raw_text = String::from_utf8_lossy(&response).to_string();

    if let Ok(entries) = serde_json::from_slice::<Vec<FsEntry>>(&response) {
        return Ok(entries);
    }

    let val: Value = serde_json::from_str(&raw_text).map_err(|e| {
        println!(
            "[list_directory:{}] final parse into Value failed: {}",
            id, e
        );
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to parse response as JSON Value: {}", e),
        )
    })?;

    if val.is_array() {
        let entries: Vec<FsEntry> = serde_json::from_value(val).map_err(|e| {
            println!(
                "[list_directory:{}] from_value into Vec<FsEntry> failed: {}",
                id, e
            );
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        return Ok(entries);
    }

    if let Value::String(s) = val {
        let inner = s.trim();
        let entries: Vec<FsEntry> = serde_json::from_str(inner).map_err(|e| {
            println!(
                "[list_directory:{}] parsing inner string into Vec<FsEntry> failed: {}",
                id, e
            );
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        return Ok(entries);
    }

    if let Value::Object(mut map) = val {
        if let Some(data_val) = map.remove("data") {
            if data_val.is_array() || data_val.is_object() {
                let entries: Vec<FsEntry> = serde_json::from_value(data_val).map_err(|e| {
                    println!(
                        "[list_directory:{}] parsing data field into Vec<FsEntry> failed: {}",
                        id, e
                    );
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                })?;
                return Ok(entries);
            }
            if let Value::String(s) = data_val {
                let inner = s.trim();
                let entries: Vec<FsEntry> = serde_json::from_str(inner).map_err(|e| {
                        println!("[list_directory:{}] parsing inner data string into Vec<FsEntry> failed: {}", id, e);
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                    })?;
                return Ok(entries);
            }
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Failed to parse directory listing into Vec<FsEntry>",
    ))
}

#[async_trait::async_trait]
impl FsType for TcpFs {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    async fn get_files_content(
        &mut self,
        file_chunk: FileChunk,
    ) -> std::io::Result<IncomingMessage> {
        let id = self
            .send_request(FileRequestPayload::FileChunk(file_chunk))
            .await?;

        let response_chunks = self.recv_response(id).await?;

        for (i, chunk) in response_chunks.iter().enumerate() {
            let preview = String::from_utf8_lossy(chunk);

            if let Ok(val) = serde_json::from_slice::<Value>(chunk) {
                if let Some(msg) = val.get("message").and_then(|v| v.as_str()) {
                    return Ok(IncomingMessage {
                        message_type: "file_content".to_string(),
                        message: msg.to_string(),
                        authcode: "0".to_string(),
                    });
                }
                if let Some(data_val) = val.get("data") {
                    if let Some(s) = data_val.as_str() {
                        return Ok(IncomingMessage {
                            message_type: "file_content".to_string(),
                            message: s.to_string(),
                            authcode: "0".to_string(),
                        });
                    } else {
                        let text = serde_json::to_string(data_val).unwrap_or_default();
                        return Ok(IncomingMessage {
                            message_type: "file_content".to_string(),
                            message: text,
                            authcode: "0".to_string(),
                        });
                    }
                }
                if let Value::String(s) = val {
                    return Ok(IncomingMessage {
                        message_type: "file_content".to_string(),
                        message: s,
                        authcode: "0".to_string(),
                    });
                }
            } else if let Ok(as_str) = std::str::from_utf8(chunk) {
                return Ok(IncomingMessage {
                    message_type: "file_content".to_string(),
                    message: as_str.to_string(),
                    authcode: "0".to_string(),
                });
            }
        }

        let response_all: Vec<u8> = response_chunks.concat();
        let message_str = String::from_utf8_lossy(&response_all).to_string();

        Ok(IncomingMessage {
            message_type: "file_content".to_string(),
            message: message_str,
            authcode: "0".to_string(),
        })
    }
    
    async fn get_path_from_tag(&mut self, tag: &str) -> std::io::Result<Vec<String>> {
        let id = self
            .send_request(FileRequestPayload::PathFromTag {
                path: "".to_string(),
                tag: Some(tag.to_string()),
            })
            .await?;

        let response_chunks = self.recv_response(id).await?;
        for chunk in response_chunks.iter() {
            if let Ok(basic_path) = serde_json::from_slice::<BasicPath>(chunk) {
                return Ok(basic_path.paths);
            }

            if let Ok(val) = serde_json::from_slice::<Value>(chunk) {
                if let Some(data_val) = val.get("data") {
                    if data_val.is_object() || data_val.is_array() {
                        if let Ok(basic_path) =
                            serde_json::from_value::<BasicPath>(data_val.clone())
                        {
                            return Ok(basic_path.paths);
                        }
                    }

                    if let Some(s) = data_val.as_str() {
                        if let Ok(basic_path) = serde_json::from_str::<BasicPath>(s) {
                            return Ok(basic_path.paths);
                        }
                    }
                }

                if let Value::String(s) = &val {
                    if let Ok(basic_path) = serde_json::from_str::<BasicPath>(s) {
                        return Ok(basic_path.paths);
                    }
                }

                if let Ok(basic_path) = serde_json::from_value::<BasicPath>(val.clone()) {
                    return Ok(basic_path.paths);
                }
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed get the path for tag '{}'", tag),
        ))
    }
    
    async fn get_metadata(&mut self, path: &str) -> std::io::Result<FsMetadata> {
        let id = self
            .send_request(FileRequestPayload::Metadata {
                path: path.to_string(),
            })
            .await?;

        let response_chunks = self.recv_response(id).await?;

        for chunk in response_chunks.iter() {
            if let Ok(meta) = serde_json::from_slice::<FsMetadata>(chunk) {
                return Ok(meta);
            }

            if let Ok(val) = serde_json::from_slice::<Value>(chunk) {
                if let Some(data_val) = val.get("data") {
                    if data_val.is_object() || data_val.is_array() {
                        if let Ok(meta) = serde_json::from_value::<FsMetadata>(data_val.clone()) {
                            return Ok(meta);
                        }
                    }

                    if let Some(s) = data_val.as_str() {
                        if let Ok(meta) = serde_json::from_str::<FsMetadata>(s) {
                            return Ok(meta);
                        }
                    }
                }

                if let Value::String(s) = &val {
                    if let Ok(meta) = serde_json::from_str::<FsMetadata>(s) {
                        return Ok(meta);
                    }
                }

                if let Ok(meta) = serde_json::from_value::<FsMetadata>(val.clone()) {
                    return Ok(meta);
                }
            }
        }

        println!(
            "[get_metadata:{}] final parse error: missing field `is_file` at line 1 column 58",
            id
        );

        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to parse metadata response for path '{}'", path),
        ))
    }
    
    async fn list_directory_within_range(
        &mut self,
        path: &str,
        start: Option<u64>,
        end: Option<u64>,
    ) -> std::io::Result<Vec<FsEntry>> {
        let id = self
            .send_request(FileRequestPayload::ListDirWithRange {
                path: path.to_string(),
                start,
                end,
            })
            .await?;

        let response_chunks = match self.recv_response(id).await {
            Ok(b) => b,
            Err(e) => {
                println!("[list_directory:{}] recv_response error: {}", id, e);
                return Err(e);
            }
        };
        parse_directory_response(&response_chunks, id)
    }

    async fn list_directory(&mut self, path: &str) -> std::io::Result<Vec<FsEntry>> {
        let id = self
            .send_request(FileRequestPayload::ListDir {
                path: path.to_string(),
            })
            .await?;

        let response_chunks = match self.recv_response(id).await {
            Ok(b) => b,
            Err(e) => {
                println!("[list_directory:{}] recv_response error: {}", id, e);
                return Err(e);
            }
        };

        parse_directory_response(&response_chunks, id)
    }
}

#[derive(Debug, Clone)]
pub struct RemoteFileSystem<S: FsType> {
    path: String,
    state: Option<S>,
    cached_metadata: Option<FsMetadata>,
    cached_entries: Option<Vec<FsEntry>>,
}

impl<S: FsType> RemoteFileSystem<S> {
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.state.as_ref()?.as_any().downcast_ref::<T>()
    }

    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.state.as_mut()?.as_any_mut().downcast_mut::<T>()
    }

    pub fn new(path: &str, state: Option<S>) -> Self {
        Self {
            path: path.to_string(),
            state,
            cached_metadata: None,
            cached_entries: None,
        }
    }

    pub fn parent(&self) -> Option<Self> {
        let parent_path = Path::new(&self.path).parent()?.to_path_buf();
        Some(Self::new(
            &parent_path.to_string_lossy(),
            self.state.clone(),
        ))
    }

    pub fn to_path_buf(&self) -> std::path::PathBuf {
        Path::new(&self.path).to_path_buf()
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.path)
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> Self {
        let mut new_path = self.path.clone();
        let path_str = path.as_ref().to_str().unwrap_or("");

        if !new_path.ends_with('/') && !path_str.starts_with('/') {
            new_path.push('/');
        }

        new_path.push_str(path_str);
        Self::new(&new_path, self.state.clone())
    }

    pub fn starts_with<P: AsRef<Path>>(&self, base: P) -> bool {
        let base_path = base.as_ref().to_str().unwrap_or("");
        let starts = self.path.starts_with(base_path);
        starts
    }

    pub async fn ensure_metadata(&mut self) -> std::io::Result<()> {
        if self.cached_metadata.is_none() {
            if let Some(state) = &mut self.state {
                self.cached_metadata = Some(state.get_metadata(&self.path).await?);
            }
        }
        Ok(())
    }

    pub async fn ensure_entries(&mut self) -> std::io::Result<()> {
        if self.cached_entries.is_none() {
            if let Some(state) = &mut self.state {
                let dir_item_count = state
                    .get_metadata(&self.path)
                    .await?
                    .optional_folder_children
                    .unwrap_or(0);

                let chunk_size = ((dir_item_count as f64) / 2.0).ceil() as u64;

                let mut all_entries = Vec::new();
                let mut start = 0;

                while start < dir_item_count {
                    let end = (start + chunk_size).min(dir_item_count);

                    let mut entries = state
                        .list_directory_within_range(&self.path, Some(start), Some(end))
                        .await?;

                    all_entries.append(&mut entries);

                    start = end;
                }

                self.cached_entries = Some(all_entries);
            } else {
                println!("[RemoteFileSystem] No state instance available to fetch entries");
            }
        }
        Ok(())
    }

    pub async fn is_dir(&mut self) -> std::io::Result<bool> {
        self.ensure_metadata().await?;
        let is_dir = self
            .cached_metadata
            .as_ref()
            .map(|m| m.is_dir)
            .unwrap_or(false);
        Ok(is_dir)
    }

    pub async fn is_file(&mut self) -> std::io::Result<bool> {
        self.ensure_metadata().await?;
        let is_file = self
            .cached_metadata
            .as_ref()
            .map(|m| m.is_file)
            .unwrap_or(false);
        Ok(is_file)
    }

    pub async fn canonicalize(&mut self) -> std::io::Result<Self> {
        self.ensure_metadata().await?;
        let canonical_path = self
            .cached_metadata
            .as_ref()
            .map(|m| m.canonical_path.clone())
            .unwrap_or_else(|| self.path.clone());

        Ok(Self::new(&canonical_path, self.state.clone()))
    }

    pub fn to_string(&self) -> String {
        self.path.clone()
    }

    pub fn file_name(&self) -> Option<std::ffi::OsString> {
        let name = Path::new(&self.path).file_name().map(|s| s.to_os_string());
        name
    }
}

impl RemoteFileSystem<TcpFs> {
    pub async fn read_dir(&self) -> std::io::Result<Vec<RemoteFileSystem<TcpFs>>> {
        let mut fs_clone = self.clone();
        fs_clone.ensure_entries().await?;

        let entries = fs_clone.cached_entries.as_ref().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "No entries cached after ensure_entries",
            )
        })?;

        let mut result = Vec::new();
        for entry in entries {
            let child_path = if self.path.ends_with('/') {
                format!("{}{}", self.path, entry.name)
            } else {
                format!("{}/{}", self.path, entry.name)
            };
            result.push(RemoteFileSystem::new(&child_path, self.state.clone()));
        }

        Ok(result)
    }
}

pub async fn get_metadata(path: &str) -> std::io::Result<FsMetadata> {
    let metadata = fs::metadata(path).await?;
    let canonical = fs::canonicalize(path).await?;

    let optional_folder_children = if metadata.is_dir() {
        let mut count = 0;
        let mut dir = fs::read_dir(path).await?;
        while let Some(entry) = dir.next_entry().await? {
            count += 1;
        }
        Some(count)
    } else {
        None
    };

    Ok(FsMetadata {
        is_file: metadata.is_file(),
        is_dir: metadata.is_dir(),
        optional_folder_children,
        canonical_path: canonical.to_string_lossy().to_string(),
    })
}

pub async fn list_directory_with_range(
    path: &str,
    start: Option<u64>,
    end: Option<u64>,
) -> std::io::Result<Vec<FsEntry>> {
    let mut entries = Vec::new();
    let mut dir = fs::read_dir(path).await?;

    let start_idx = start.unwrap_or(0);
    let mut current_idx = 0u64;

    while let Some(entry) = dir.next_entry().await? {
        if current_idx < start_idx {
            current_idx += 1;
            continue;
        }

        if let Some(end_idx) = end {
            if current_idx >= end_idx {
                break;
            }
        }

        let metadata = entry.metadata().await?;
        entries.push(FsEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
        });

        current_idx += 1;
    }

    Ok(entries)
}

pub async fn list_directory(path: &str) -> std::io::Result<Vec<FsEntry>> {
    list_directory_with_range(path, None, None).await
}

pub async fn get_files_content(file_chunk: FileChunk) -> std::io::Result<MessagePayload> {
    let metadata = fs::metadata(&file_chunk.file_name).await?;

    if metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Path is a directory: {}", file_chunk.file_name),
        ));
    }

    let mut file = File::open(&file_chunk.file_name).await?;
    let offset: u64 = file_chunk
        .file_chunk_offet
        .parse()
        .expect("Invalid file offset");

    file.seek(SeekFrom::Start(offset.try_into().unwrap()))
        .await?;
    let chunk_size: usize = file_chunk
        .file_chunk_size
        .parse()
        .expect("Invalid chunk size");

    let mut buffer = vec![0; chunk_size];
    let bytes_read = file.read(&mut buffer).await?;
    let content = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();

    Ok(MessagePayload {
        r#type: "file_content".to_string(),
        message: content,
        authcode: "0".to_string(),
    })
}

pub async fn handle_multipart_message(
    payload: &MessagePayload,
    current_file: &mut Option<File>,
) -> std::io::Result<()> {
    match payload.r#type.as_str() {
        "start_file" => {
            let file_name = format!("server/{}", payload.message);
            tokio::fs::create_dir_all("server").await?;
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(&file_name)
                .await?;
            *current_file = Some(file);
            println!(
                "[handle_multipart_message] Started writing file: {}",
                file_name
            );
        }
        "end_file" => {
            if let Some(mut file) = current_file.take() {
                file.flush().await?;
                println!(
                    "[handle_multipart_message] Finished writing file: {}",
                    payload.message
                );
            } else {
                eprintln!(
                    "[handle_multipart_message] end_file received but no file is currently open"
                );
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum FileOperations {
    FileDownloadOperation(String),
    FileMoveOperation(String),
    FileZipOperation(String),
    FileUnzipOperation(String),
    FileCopyOperation(String),
    Unknown
}

impl FileOperations {
    pub fn as_inner_str(&self) -> Option<&str> {
        match self {
            FileOperations::FileDownloadOperation(s) => Some(s),
            FileOperations::FileZipOperation(s) => Some(s),
            FileOperations::FileMoveOperation(s) => Some(s),
            FileOperations::FileUnzipOperation(s) => Some(s),
            FileOperations::FileCopyOperation(s) => Some(s),
            _ => None
        }
    }
}

impl fmt::Display for FileOperations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FileOperations::FileDownloadOperation(_) => "FileDownloadOperation",
            FileOperations::FileZipOperation(_) => "FileZipOperation",
            FileOperations::FileMoveOperation(_) => "FileMoveOperation",
            FileOperations::FileUnzipOperation(_) => "FileUnzipOperation",
            FileOperations::FileCopyOperation(_) => "FileCopyOperation",
            _ => "not implemented",
        };
        write!(f, "{}", s)
    }
}

pub fn execute_file_operation(encoded_src: FileOperations, encoded_dest: FileOperations, dir: String) -> std::io::Result<()> {
    let dest = encoded_dest.as_inner_str().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid destination"))?;
    let src = encoded_src.as_inner_str().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid source"))?;
    let src_path = PathBuf::from(&dir).join(src);
    let dest_path = PathBuf::from(&dir).join(dest);
    
    match &encoded_src {
        FileOperations::FileCopyOperation(_) => {
            let final_dest = if dest_path.exists() && dest_path.is_dir() {
                if let Some(filename) = src_path.file_name() {
                    dest_path.join(filename)
                } else {
                    dest_path
                }
            } else {
                dest_path
            };
            std::fs::copy(&src_path, &final_dest)?;
        },
        FileOperations::FileMoveOperation(_) => {
            let final_dest = if dest_path.exists() && dest_path.is_dir() {
                if let Some(filename) = src_path.file_name() {
                    dest_path.join(filename)
                } else {
                    dest_path
                }
            } else {
                dest_path
            };
            std::fs::rename(&src_path, &final_dest)?;
        },
        FileOperations::FileZipOperation(_) => {
            let mut final_dest = if dest_path.exists() && dest_path.is_dir() {
                if let Some(filename) = src_path.file_name() {
                    dest_path.join(filename)
                } else {
                    dest_path
                }
            } else {
                dest_path
            };
            final_dest.set_extension("zip");
            
            let file = std::fs::File::create(&final_dest)?;
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            
            if src_path.is_dir() {
                let dir_name = src_path.file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid directory name"))?;
                
                fn zip_directory(zip: &mut zip::ZipWriter<std::fs::File>, path: &Path, prefix: &str, options: zip::write::SimpleFileOptions) -> std::io::Result<()> {
                    for entry in std::fs::read_dir(path)? {
                        let entry = entry?;
                        let entry_path = entry.path();
                        let name = entry_path.file_name()
                            .and_then(|n| n.to_str())
                            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid filename"))?;
                        let zip_path = format!("{}/{}", prefix, name);
                        
                        if entry_path.is_dir() {
                            zip.add_directory(&zip_path, options)?;
                            zip_directory(zip, &entry_path, &zip_path, options)?;
                        } else {
                            zip.start_file(&zip_path, options)?;
                            let mut f = std::fs::File::open(&entry_path)?;
                            std::io::copy(&mut f, zip)?;
                        }
                    }
                    Ok(())
                }
                
                zip.add_directory(dir_name, options)?;
                zip_directory(&mut zip, &src_path, dir_name, options)?;
            } else {
                let filename = src_path.file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid filename"))?;
                zip.start_file(filename, options)?;
                let mut src_file = std::fs::File::open(&src_path)?;
                std::io::copy(&mut src_file, &mut zip)?;
            }
            
            zip.finish()?;
        },
        FileOperations::FileUnzipOperation(_) => {
            if !src_path.exists() {
                return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Source zip file not found"));
            }
            
            let file = std::fs::File::open(&src_path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            
            let extract_dir = if dest_path.exists() && dest_path.is_dir() {
                dest_path
            } else {
                std::fs::create_dir_all(&dest_path)?;
                dest_path
            };
            
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let outpath = extract_dir.join(file.name());
                let is_directory = file.is_dir() || (file.size() == 0 && !file.name().contains('.'));
                
                if is_directory {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            std::fs::create_dir_all(p)?;
                        }
                    }
                    let mut outfile = std::fs::File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
        },
        _ => {}
    }
    Ok(())
}