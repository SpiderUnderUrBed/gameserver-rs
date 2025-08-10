// struct FsItem {
//     name:
// }
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::timeout;
use async_trait::async_trait;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
pub enum FsItem {
    File(String),
    Folder(String),
}
#[async_trait::async_trait]
pub trait FsType: Clone + Send + Sync {
    async fn get_metadata(&mut self, path: &str) -> std::io::Result<FsMetadata>;
    async fn list_directory(&mut self, path: &str) -> std::io::Result<Vec<FsEntry>>;
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

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum FileRequestPayload {
    Metadata { path: String },
    ListDir { path: String },
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

// TCP Filesystem implementation
#[derive(Debug)]
pub struct TcpFs {
    pub tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    pub tcp_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    pub request_id: Arc<AtomicU64>,
}

impl Clone for TcpFs {
    fn clone(&self) -> Self {
        println!(
            "[TcpFs] Cloning TcpFs. Receiver count before clone: {}",
            self.tcp_tx.receiver_count()
        );
        let cloned = TcpFs {
            tcp_tx: self.tcp_tx.clone(),
            tcp_rx: self.tcp_tx.subscribe(),
            request_id: self.request_id.clone(),
        };
        println!(
            "[TcpFs] Cloned TcpFs. Receiver count after clone: {}",
            cloned.tcp_tx.receiver_count()
        );
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

    // Send request once, no waiting for responses here
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
        let timeout_duration = Duration::from_secs(10);
        let start_time = Instant::now();

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
            println!(
                "[TcpFs][recv_response] Raw response for id {}: {}",
                id, response_str
            );

            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&response_str) {
                // Accept both "in_response_to" and "id"
                let msg_id = json_value
                    .get("in_response_to")
                    .and_then(|v| v.as_u64())
                    .or_else(|| json_value.get("id").and_then(|v| v.as_u64()));

                if let Some(mid) = msg_id {
                    if mid != id {
                        continue;
                    }

                    if let Some(data) = json_value.get("data") {
                        // If object, check if FsMetadata is complete
                        if let Some(obj) = data.as_object() {
                            if obj.contains_key("is_file") && obj.contains_key("is_dir") {
                                return Ok(serde_json::to_vec(data)?);
                            } else {
                                println!(
                                    "[TcpFs][recv_response] Skipping incomplete metadata: {:?}",
                                    obj
                                );
                                continue;
                            }
                        }

                        // If data is a byte array
                        if let Some(bytes_array) = data.as_array() {
                            if bytes_array.iter().all(|v| v.is_u64()) {
                                let raw_bytes: Vec<u8> = bytes_array
                                    .iter()
                                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                                    .collect();

                                // Try UTF-8 decoding
                                if let Ok(mut inner_str) = String::from_utf8(raw_bytes.clone()) {
                                    // Keep unwrapping JSON strings until we hit object/array
                                    loop {
                                        if let Ok(inner_json) =
                                            serde_json::from_str::<serde_json::Value>(&inner_str)
                                        {
                                            if inner_json.is_string() {
                                                inner_str =
                                                    inner_json.as_str().unwrap().to_string();
                                                continue;
                                            } else {
                                                return Ok(serde_json::to_vec(&inner_json)?);
                                            }
                                        }
                                        break;
                                    }
                                }

                                return Ok(raw_bytes);
                            }
                        }

                        // If data is a JSON string
                        if let Some(s) = data.as_str() {
                            let mut inner_str = s.to_string();
                            loop {
                                if let Ok(inner_json) =
                                    serde_json::from_str::<serde_json::Value>(&inner_str)
                                {
                                    if inner_json.is_string() {
                                        inner_str = inner_json.as_str().unwrap().to_string();
                                        continue;
                                    } else {
                                        return Ok(serde_json::to_vec(&inner_json)?);
                                    }
                                }
                                break;
                            }
                            return Ok(s.as_bytes().to_vec());
                        }

                        // If data is already JSON object or array
                        if data.is_object() || data.is_array() {
                            return Ok(serde_json::to_vec(data)?);
                        }
                    }
                }
            }

            // Fallbacks — try direct parsing
            if let Ok(entries) = serde_json::from_slice::<Vec<FsEntry>>(&response) {
                return Ok(serde_json::to_vec(&entries)?);
            }
            if let Ok(entry) = serde_json::from_slice::<FsEntry>(&response) {
                return Ok(serde_json::to_vec(&entry)?);
            }
            if let Ok(metadata) = serde_json::from_slice::<FsMetadata>(&response) {
                return Ok(serde_json::to_vec(&metadata)?);
            }
        }
    }
}

#[async_trait::async_trait]
impl FsType for TcpFs {
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
                println!(
                    "[TcpFs][get_metadata] Failed to parse FsMetadata JSON: {}",
                    e
                );
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }
        }
    }

    async fn list_directory(&mut self, path: &str) -> std::io::Result<Vec<FsEntry>> {
        let metadata = self.get_metadata(path).await?;

        if metadata.is_file {
            println!("[TcpFs] Error: path is a file, not a directory");
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path is a file, not a directory",
            ));
        }

        let expected_count = metadata.optional_folder_children.unwrap_or(0);

        if expected_count == 0 {
            let id = self
                .send_request(FileRequestPayload::ListDir {
                    path: path.to_string(),
                })
                .await?;

            let response = self.recv_response(id).await?;

            let list: Vec<FsEntry> = serde_json::from_slice(&response).unwrap_or_default();

            return Ok(list);
        }

        let id = self
            .send_request(FileRequestPayload::ListDir {
                path: path.to_string(),
            })
            .await?;

        let mut collected_entries = Vec::new();

        // Timer duration for timeout after no new entries
        let timeout_duration = Duration::from_secs(3);
        // Keep track of when last new entries arrived
        let mut last_received = Instant::now();

        loop {
            // If we've got all entries, break
            if (collected_entries.len() as u64) >= expected_count {
                break;
            }

            // Calculate remaining time before timeout triggers
            let time_since_last = Instant::now().duration_since(last_received);
            let time_left = if time_since_last >= timeout_duration {
                // Timeout already expired
                Duration::ZERO
            } else {
                timeout_duration - time_since_last
            };

            // Wait for next response chunk with timeout
            let recv_result = timeout(time_left, self.recv_response(id)).await;

            match recv_result {
                Ok(Ok(response)) => {
                    let mut added_entries = 0;

                    if let Ok(mut entries) = serde_json::from_slice::<Vec<FsEntry>>(&response) {
                        added_entries = entries.len();
                        collected_entries.append(&mut entries);
                    } else if let Ok(single) = serde_json::from_slice::<FsEntry>(&response) {
                        added_entries = 1;
                        collected_entries.push(single);
                    }

                    if added_entries > 0 {
                        last_received = Instant::now();
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("[TcpFs] Error receiving response: {}", e);
                    break;
                }
                Err(_) => {
                    // Timeout triggered, no new entries within timeout_duration
                    println!(
                        "[TcpFs] Timeout waiting for new entries, returning collected entries"
                    );
                    break;
                }
            }
        }

        Ok(collected_entries)
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
    pub fn new(path: &str, state: Option<S>) -> Self {
        Self {
            path: path.to_string(),
            state,
            cached_metadata: None,
            cached_entries: None,
        }
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

// Specific implementation for TcpFs
impl RemoteFileSystem<TcpFs> {
    pub async fn read_dir(&self) -> std::io::Result<Vec<RemoteFileSystem<TcpFs>>> {
        // Create a mutable clone to call ensure_entries
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