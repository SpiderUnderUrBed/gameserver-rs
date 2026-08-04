use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub use networked_filesystem::*;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Notify;
// use serde_json::Value;

#[allow(dead_code)]
pub struct LocalState {
    current_directory: Vec<FsItem>,
}

#[allow(dead_code)]
pub struct FileOperations {
    state: HashMap<u64, LocalState>,
    update: Notify,
}
impl FileOperations {
    pub fn new() -> FileOperations {
        FileOperations {
            state: HashMap::new(),
            update: Notify::new(),
        }
    }
}
pub enum FileOperationResult {
    InvalidOperation,
}
// TODO: consider a response associated type?
pub trait FileRequestExecutable {
    type Request;
    fn execute_bytes(&self) -> Result<Vec<u8>, FileOperationResult>;
    fn try_from_slice(id: Option<u8>, body: Vec<u8>) -> Result<Self::Request, serde_json::Error>;
    // fn try_from_str(id: Option<u8>, body: &str) -> Result<Self::Request, serde_json::Error>;
    // fn to_string(&self) -> Result<String, serde_json::Error>;
    fn item_id() -> u8;
    fn to_bytes(&self) -> Vec<u8>;
}

pub trait FileResponseExecuatable {
    type Request;
    fn try_from_str(id: Option<u8>, body: &str) -> Result<Self::Request, serde_json::Error>;
    fn item_id() -> u8;
    fn set_state_id(&mut self, id: u8);
    // fn to_string(&self) -> Result<String, serde_json::Error>;
}

#[derive(Deserialize, Serialize)]
pub struct CannonolizeRequest {
    location: String,
}

#[derive(Serialize, Deserialize)]
struct FsItem {
    name: String,
    is_dir: bool,
}

#[derive(Serialize, Deserialize)]
struct DirectoryResponse {
    id: u8,
    directory: Vec<FsItem>,
}
impl FileResponseExecuatable for DirectoryResponse {
    type Request = Self;

    fn try_from_str(id: Option<u8>, body: &str) -> Result<Self::Request, serde_json::Error> {
        let mut request = serde_json::from_str::<Self>(body)?;
        request.id = id.unwrap_or(0);
        Ok(request)
    }
    fn item_id() -> u8 {
        1
    }
    fn set_state_id(&mut self, id: u8) {
        self.id = id;
    }
    // fn to_string(&self) -> Result<String, serde_json::Error> {
    //     Ok(format!("0{}{}", self.id, serde_json::to_string(self)?))
    // }
}

// fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry)) -> io::Result<()> {
//     if dir.is_dir() {
//         for entry in fs::read_dir(dir)? {
//             let entry = entry?;
//             let path = entry.path();
//             if path.is_dir() {
//                 visit_dirs(&path, cb)?;
//             } else {
//                 cb(&entry);
//             }
//         }
//     }
//     Ok(())
// }

#[derive(Deserialize, Serialize)]
pub struct LsRequest {
    pub id: u8,
    pub location: String,
}
impl FileRequestExecutable for LsRequest {
    type Request = Self;
    fn execute_bytes(&self) -> Result<Vec<u8>, FileOperationResult> {
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
                        // if let Some(name) = path.file_name(){
                        //     name.to_str().unwrap()
                        // } else {
                        //     path.to_str().unwrap().to_string()
                        // }
                    },
                    is_dir: path.is_dir(),
                })
            }
            let mut bytes = Vec::new();
            bytes.push(DirectoryResponse::item_id());
            bytes.push(self.id);
            bytes.extend(serde_json::to_vec(&response).unwrap());
            Ok(bytes)
        } else {
            return Err(FileOperationResult::InvalidOperation);
        }
    }
    // fn try_from_str(id: Option<u8>, body: &str) -> Result<Self::Request, serde_json::Error> {
    //     let mut request = serde_json::from_str::<Self>(body)?;
    //     request.id = id.unwrap_or(0);
    //     Ok(request)
    // }
    fn try_from_slice(id: Option<u8>, body: Vec<u8>) -> Result<Self::Request, serde_json::Error> {
        let mut request = serde_json::from_slice::<Self>(&body)?;
        request.id = id.unwrap_or(0);
        Ok(request)
    }
    // fn to_string(&self) -> Result<String, serde_json::Error> {
    //     Ok(format!("0{}{}", self.id, serde_json::to_string(self)?))
    // }

    fn item_id() -> u8 {
        0
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(LsRequest::item_id());
        bytes.push(self.id);
        bytes.extend(serde_json::to_vec(self).unwrap());
        bytes
    }
}

#[derive(Deserialize, Serialize)]
pub enum FileRequestErrors {
    NoRequestHeader,
    InvalidRequestHeader,
    InvalidBody,
    CouldNotParse,
}

// TODO: in the future I want something better like tagging structs with byte headers,
// perferably in the Serialization and Deserialization layer
pub struct FileRequest {}
impl FileRequest {
    pub fn decode_into<S: Serialize + DeserializeOwned + FileResponseExecuatable>(
        encoding: Vec<u8>,
    ) -> Result<S, FileRequestErrors> {
        // let (request_type, request_remainder) = encoding.split_at(1);
        // let (state_id, request_body) = request_remainder.split_at(1);
        let request_type = encoding
            .get(0)
            .ok_or_else(|| FileRequestErrors::CouldNotParse)?;
        let state_id = encoding
            .get(1)
            .ok_or_else(|| FileRequestErrors::CouldNotParse)?;
        let (_, request_body) = encoding
            .split_at_checked(2)
            .ok_or_else(|| FileRequestErrors::CouldNotParse)?;

        if S::item_id() == *request_type {
            if let Ok(mut value) = serde_json::from_slice::<S>(&request_body) {
                value.set_state_id(*state_id);
                Ok(value)
            } else {
                Err(FileRequestErrors::CouldNotParse)
            }
        } else {
            Err(FileRequestErrors::InvalidRequestHeader)
        }
    }
    // pub fn from_value(value: Value) -> Result<impl FileRequestExecutable, ()> {
    //     // match serde_json::from_value::<LsRequest>(value){
    //     //     Ok(_) => todo!(),
    //     //     Err(E) => todo!(),
    //     // }
    //     if let Ok(ls_request) = serde_json::from_value::<LsRequest>(value){
    //         Ok(ls_request)
    //     } else {
    //         Err(())
    //     }
    // }
    // pub fn decode_into<S: Serialize + DeserializeOwned + FileResponseExecuatable>(encoding: String) -> Result<S, FileRequestErrors> {
    //     let (request_type_str, request_remainder) = encoding.split_at(1);
    //     let request_type = match request_type_str.parse::<u8>() {
    //         Ok(request_type) => request_type,
    //         Err(_) => return Err(FileRequestErrors::InvalidRequestHeader),
    //     };
    //     let (state_id_str, request_body) = request_remainder.split_at(1);
    //     let state_id = match state_id_str.parse::<u8>(){
    //         Ok(state_id) => state_id,
    //         Err(_) => return Err(FileRequestErrors::InvalidRequestHeader),
    //     };
    //     if S::item_id() == request_type {
    //         if let Ok(mut value) = serde_json::from_str::<S>(&request_body){
    //             value.set_state_id(state_id);
    //             Ok(value)
    //         } else {
    //             Err(FileRequestErrors::CouldNotParse)
    //         }
    //     } else {
    //         Err(FileRequestErrors::InvalidRequestHeader)
    //     }
    // }
    //  pub fn encode_into<S: Serialize + DeserializeOwned + FileRequestExecutable>(encoding: String) -> Result<S, FileRequestErrors> {
    //     // S::try_from_str(Some(state_id), request_body).map_err(|_| FileRequestErrors::InvalidBody)?
    //  }
    pub fn from_request(
        encoding: Vec<u8>,
    ) -> Result<impl FileRequestExecutable, FileRequestErrors> {
        let request_type = encoding
            .get(0)
            .ok_or_else(|| FileRequestErrors::CouldNotParse)?;
        let state_id = encoding
            .get(1)
            .ok_or_else(|| FileRequestErrors::CouldNotParse)?;
        let (_, request_body) = encoding
            .split_at_checked(2)
            .ok_or_else(|| FileRequestErrors::CouldNotParse)?;

        match request_type {
            0 => Ok(
                LsRequest::try_from_slice(Some(*state_id), request_body.to_vec())
                    .map_err(|_| FileRequestErrors::InvalidBody)?,
            ),
            _ => return Err(FileRequestErrors::NoRequestHeader),
        }
    }
}
