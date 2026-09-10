use std::any::Any;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Output;
pub use networked_filesystem::*;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;

#[allow(dead_code)]
#[derive(Clone)]
pub struct FileOperations {
    state: HashMap<u64, LocalState>,
    //update: Notify,
}
impl FileOperations {
    pub fn new() -> FileOperations {
        FileOperations {
            state: HashMap::new(),
            //update: Notify::new(),
        }
    }

    pub fn from_known_request_bytes<S: FileRequestExecutable + 'static>(
        encoding: Vec<u8>,
    ) -> Result<Box<dyn FileRequestExecutable<Output = S::Output>>, FileRequestErrors> {
        let request = serde_json::from_slice::<Box<dyn FileRequest>>(&encoding)
            .map_err(|_| FileRequestErrors::CouldNotParse)?;

        match request.into_any().downcast::<S>() {
            Ok(request) => Ok(request as Box<dyn FileRequestExecutable<Output = S::Output>>),
            Err(_) => Err(FileRequestErrors::CouldNotFindRequest),
        }
    }
    pub fn from_tagged_request(
        encoding: Vec<u8>,
    ) -> Result<Box<dyn FileRequest>, FileRequestErrors> {
        serde_json::from_slice::<Box<dyn FileRequest>>(&encoding)
            .map_err(|_| FileRequestErrors::CouldNotParse)
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct FsItem {
    pub name: String,
    pub is_dir: bool,
}
#[allow(dead_code)]
#[derive(Default)]
pub struct LocalCache {
    current_directory: Vec<FsItem>,
}


#[derive(Clone)]
pub enum FileOperationResult {
    InvalidOperation,
    NoPath,
    // Any(Box<dyn Error + Send + Sync>)
}

pub trait FileRequestExecutable: Send + Sync {
    type Output;
    fn execute(&self) -> Result<Self::Output, FileOperationResult>;
}
pub trait FileRequestDecoder: Sized {
    fn try_from_slice(id: Option<u8>, body: Vec<u8>) -> Result<Self, serde_json::Error>;
}

#[typetag::serde(tag = "type")]
pub trait FileRequest: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn execute_bytes(&self) -> Result<Vec<u8>, FileOperationResult>;
}
#[typetag::serde(tag = "type")]
pub trait FileResponse {
    // fn item_id() -> u8;
    fn as_any(&self) -> &dyn Any;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DirectoryResponse {
    id: u8,
    pub directory: Vec<FsItem>,
}
#[typetag::serde]
impl FileResponse for DirectoryResponse {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CannonolizeResponse {
    pub id: u8,
    pub path: String,
}
#[typetag::serde]
impl FileResponse for CannonolizeResponse {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SizeResponse {
    pub id: u8,
    pub size: u64,
}
#[typetag::serde]
impl FileResponse for SizeResponse {
    fn as_any(&self) -> &dyn Any {
        self
    }
}


#[derive(Deserialize, Serialize)]
pub struct CannonolizeRequest {
    pub id: u8,
    pub path: String,
}
#[typetag::serde]
impl FileRequest for CannonolizeRequest {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> { 
        self 
    }
    fn execute_bytes(&self) -> Result<Vec<u8>, FileOperationResult> {
        self.execute()
            .map(|o| serde_json::to_vec(&o).unwrap())
            .map_err(|e| e.clone())
    }
}
impl FileRequestDecoder for CannonolizeRequest {
    fn try_from_slice(id: Option<u8>, body: Vec<u8>) -> Result<Self, serde_json::Error> {
        let mut request = serde_json::from_slice::<Self>(&body)?;
        request.id = id.unwrap_or(0);
        Ok(request)
    }
}
impl FileRequestExecutable for CannonolizeRequest {
    type Output = CannonolizeResponse;
    fn execute(&self) -> Result<Self::Output, FileOperationResult>{
        let path_buf = fs::canonicalize(self.path.clone())
            .map_err(|_| FileOperationResult::InvalidOperation)?;
        if let Some(path) = path_buf.to_str(){
            let response = CannonolizeResponse {
                id: self.id,
                path: path.to_owned()
            };
            //serde_json::to_vec(&response).unwrap()
            Ok(response)
        } else {
            Err(FileOperationResult::InvalidOperation)
        }
    }
}
#[derive(Deserialize, Serialize)]
pub struct LsRequest {
    pub id: u8,
    pub location: String,
}

#[typetag::serde]
impl FileRequest for LsRequest {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> { 
        self 
    }
    fn execute_bytes(&self) -> Result<Vec<u8>, FileOperationResult> {
        self.execute()
            .map(|o| serde_json::to_vec(&o).unwrap())
            .map_err(|e| e.clone())
    }
}

impl FileRequestExecutable for LsRequest {
    type Output = DirectoryResponse;

    fn execute(&self) -> Result<DirectoryResponse, FileOperationResult> {
        let path = Path::new(&self.location);
        if path.is_dir() {
            let mut response = DirectoryResponse {
                id: self.id,
                directory: Vec::new(),
            };
            for entry in fs::read_dir(path).map_err(|_| FileOperationResult::InvalidOperation)? {
                let entry = entry.map_err(|_| FileOperationResult::InvalidOperation)?;
                let path = entry.path();
                response.directory.push(FsItem {
                    name: {
                        path.file_name().unwrap().to_string_lossy().to_string()
                    },
                    is_dir: path.is_dir(),
                })
            }
            Ok(response)
        } else {
            return Err(FileOperationResult::InvalidOperation);
        }
    }
}
impl FileRequestDecoder for LsRequest {
    fn try_from_slice(id: Option<u8>, body: Vec<u8>) -> Result<Self, serde_json::Error> {
        let mut request = serde_json::from_slice::<Self>(&body)?;
        request.id = id.unwrap_or(0);
        Ok(request)
    }
}

#[derive(Deserialize, Serialize)]
pub struct SizeRequest {
    pub id: u8,
    pub location: String
}
impl FileRequestDecoder for SizeRequest {
    fn try_from_slice(id: Option<u8>, body: Vec<u8>) -> Result<Self, serde_json::Error> {
        let mut request = serde_json::from_slice::<Self>(&body)?;
        request.id = id.unwrap_or(0);
        Ok(request)
    }
}

#[typetag::serde]
impl FileRequest for SizeRequest {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> { 
        self 
    }
    fn execute_bytes(&self) -> Result<Vec<u8>, FileOperationResult> {
        self.execute()
            .map(|o| serde_json::to_vec(&o).unwrap())
            .map_err(|e| e.clone())
    }
}
impl FileRequestExecutable for SizeRequest {
    type Output = SizeResponse;

    fn execute(&self) -> Result<SizeResponse, FileOperationResult>{
        let metadata = fs::metadata(self.location.clone())
            .map_err(|_| FileOperationResult::InvalidOperation)?;

        let response = SizeResponse {
            id: self.id,
            size: metadata.len()
        };
        Ok(response)
    }
}

#[derive(Deserialize, Serialize)]
pub enum FileRequestErrors {
    NoRequestHeader,
    CouldNotFindRequest,
    InvalidRequestHeader,
    InvalidBody,
    CouldNotParse,
}
