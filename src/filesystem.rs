use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{
    any::Any,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::time::timeout;

use crate::IncomingMessage;

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
#[derive(Serialize, Deserialize)]
pub struct FileRequestMessage {
    id: u64,
    #[serde(flatten)]
    payload: FileRequestPayload,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FileChunk {
    pub(crate) file_name: String,
    pub(crate) file_chunk_offet: String,
    pub(crate) file_chunk_size: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum FileRequestPayload {
    Metadata { path: String },
    ListDir { path: String },
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
    async fn recv_response(&mut self, id: u64) -> std::io::Result<Vec<u8>> {
        use serde_json::Value;
        use std::time::Instant;
        use tokio::time::{sleep, Duration};

        let timeout_duration = Duration::from_secs(10);
        let start_time = Instant::now();

        let mut assembled = String::new();
        let mut assembling_for: Option<u64> = None;
        let mut did_micro_wait = false;

        let mut assembly_deadline: Option<Instant> = None;
        let assembly_timeout = Duration::from_millis(2000);

        fn looks_closed(s: &str) -> bool {
            let t = s.trim();
            if t.is_empty() {
                return false;
            }
            match (t.chars().next(), t.chars().rev().next()) {
                (Some('['), Some(']')) => true,
                (Some('{'), Some('}')) => true,
                _ => false,
            }
        }

        fn try_salvage_json(s: &str) -> Result<Vec<u8>, String> {
            if serde_json::from_str::<Value>(s).is_ok() {
                return Ok(s.as_bytes().to_vec());
            }
            if let (Some(start), Some(end)) = (s.find('['), s.rfind(']')) {
                if start < end {
                    let candidate = &s[start..=end];
                    if serde_json::from_str::<Value>(candidate).is_ok() {
                        return Ok(candidate.as_bytes().to_vec());
                    }
                }
            }
            if let (Some(start), Some(end)) = (s.find('{'), s.rfind('}')) {
                if start < end {
                    let candidate = &s[start..=end];
                    if serde_json::from_str::<Value>(candidate).is_ok() {
                        return Ok(candidate.as_bytes().to_vec());
                    }
                }
            }
            let snippet = if s.len() > 1024 {
                s[..1024].to_string() + "..."
            } else {
                s.to_string()
            };
            Err(snippet)
        }

        loop {
            if start_time.elapsed() > timeout_duration {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Timeout waiting for response to request {}", id),
                ));
            }

            if assembling_for == Some(id) {
                if let Some(deadline) = assembly_deadline {
                    if Instant::now() > deadline {
                        match try_salvage_json(&assembled) {
                            Ok(bytes) => return Ok(bytes),
                            Err(snippet) => {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::TimedOut,
                                    format!(
                                        "Assembly timeout for request {}: partial data: {}",
                                        id, snippet
                                    ),
                                ));
                            }
                        }
                    }
                }
            }

            let response = self
                .tcp_rx
                .recv()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionAborted, e))?;

            if response.is_empty() {
                continue;
            }

            let response_str = String::from_utf8_lossy(&response).to_string();
            if assembling_for == Some(id) || !assembled.is_empty() {
                assembled.push_str(&response_str);
                if !did_micro_wait {
                    did_micro_wait = true;
                    sleep(Duration::from_millis(30)).await;
                }

                if looks_closed(&assembled) {
                    if let Ok(_) = serde_json::from_str::<Value>(&assembled) {
                        return Ok(assembled.into_bytes());
                    }
                }
                continue;
            }

            match serde_json::from_str::<Value>(&response_str) {
                Ok(mut json_value) => {
                    if let Some(msg_id) = json_value.get("in_response_to").and_then(|v| v.as_u64())
                    {
                        if msg_id != id {
                            continue;
                        }

                        assembling_for = Some(id);
                        assembly_deadline = Some(Instant::now() + assembly_timeout);

                        if let Some(data_val) = json_value.get_mut("data") {
                            match data_val {
                                Value::String(s) => {
                                    assembled.push_str(s);
                                    if looks_closed(&assembled)
                                        && serde_json::from_str::<Value>(&assembled).is_ok()
                                    {
                                        return Ok(assembled.into_bytes());
                                    }
                                    continue;
                                }
                                Value::Array(_) | Value::Object(_) => {
                                    return serde_json::to_vec(data_val).map_err(|e| {
                                        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                                    });
                                }
                                _ => continue,
                            }
                        }
                    }
                }
                Err(_) => {
                    assembled.push_str(&response_str);
                    assembling_for = Some(id);
                    assembly_deadline = Some(Instant::now() + assembly_timeout);
                    continue;
                }
            }
        }
    }
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

        let response = self.recv_response(id).await?;

        let message_str = String::from_utf8_lossy(&response).to_string();

        Ok(IncomingMessage {
            message_type: "file_content".to_string(),
            message: message_str,
            authcode: "0".to_string(),
        })
    }

    async fn get_metadata(&mut self, path: &str) -> std::io::Result<FsMetadata> {
        let id = self
            .send_request(FileRequestPayload::Metadata {
                path: path.to_string(),
            })
            .await?;

        let response = self.recv_response(id).await?;

        match serde_json::from_slice::<FsMetadata>(&response) {
            Ok(metadata) => Ok(metadata),
            Err(e) => {
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }
        }
    }
    async fn list_directory(&mut self, path: &str) -> std::io::Result<Vec<FsEntry>> {
        use serde_json::Value;

        let id = self
            .send_request(FileRequestPayload::ListDir {
                path: path.to_string(),
            })
            .await?;

        let response = match self.recv_response(id).await {
            Ok(b) => b,
            Err(e) => {
                println!(
                    "[TcpFs][list_directory] recv_response error for id {}: {}",
                    id, e
                );
                return Err(e);
            }
        };

        let raw_text = String::from_utf8_lossy(&response).to_string();
        if let Ok(entries) = serde_json::from_slice::<Vec<FsEntry>>(&response) {
            return Ok(entries);
        }

        let val: Value = serde_json::from_str(&raw_text).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to parse response as JSON Value: {}", e),
            )
        })?;

        if val.is_array() {
            let entries: Vec<FsEntry> = serde_json::from_value(val).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })?;
            return Ok(entries);
        }

        if let Value::String(s) = val {
            let inner = s.trim();
            let entries: Vec<FsEntry> = serde_json::from_str(inner).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })?;
            return Ok(entries);
        }

        if let Value::Object(mut map) = val {
            if let Some(data_val) = map.remove("data") {
                if data_val.is_array() || data_val.is_object() {
                    let entries: Vec<FsEntry> = serde_json::from_value(data_val).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                    })?;
                    return Ok(entries);
                }
                if let Value::String(s) = data_val {
                    let inner = s.trim();
                    let entries: Vec<FsEntry> = serde_json::from_str(inner).map_err(|e| {
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
                self.cached_entries = Some(state.list_directory(&self.path).await?);
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
