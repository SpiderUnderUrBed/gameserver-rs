use std::any::Any;
use std::collections::HashMap;
use std::f32::consts::E;
use std::fs;
use std::path::Path;

pub use networked_filesystem::*;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Notify;
// use serde_json::Value;


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
    pub fn from_raw_bytes_request(
        encoding: Vec<u8>
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
            1 => Ok(
                LsRequest::try_from_slice(Some(*state_id), request_body.to_vec())
                    .map_err(|_| FileRequestErrors::InvalidBody)?,
            ),
            _ => return Err(FileRequestErrors::NoRequestHeader),
        }
    }
    pub fn from_tagged_request(
        encoding: Vec<u8>,
    ) -> Result<impl FileRequestExecutable, FileRequestErrors> {
        println!("X");
        if let Ok(request) = serde_json::from_slice::<Box<dyn FileRequest>>(&encoding){
            println!("A");
            if let Some(ls_request) = request.as_any().downcast_ref::<LsRequest>(){
                println!("B");
                // Ok(LsRequest::try_from_slice(None, encoding)
                //     .map_err(|_| FileRequestErrors::InvalidBody)?)
                // let file_request_trait: &dyn FileRequestExecutable = ls_request;
                // Ok(file_request_trait)

                // Rusts type system allows me to recreate the struct 
                // and not to return 'ls_request' directly, hence this step
                let returned_request = LsRequest {
                    id: ls_request.id,
                    location: ls_request.location.clone(),
                };
                Ok(returned_request)
            } else {
                Err(FileRequestErrors::CouldNotFindRequest)
            }
        } else {
            Err(FileRequestErrors::CouldNotParse)
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FsItem {
    name: String,
    is_dir: bool,
}
#[allow(dead_code)]
#[derive(Default)]
pub struct LocalCache {
    current_directory: Vec<FsItem>,
}


pub enum FileOperationResult {
    InvalidOperation,
}
// TODO: consider a response associated type?

pub trait FileRequestExecutable {
    type Request;
    fn execute_bytes(&self) -> Result<Vec<u8>, FileOperationResult>;
    fn execute_bytes_tagged(&self) -> Result<Vec<u8>, FileOperationResult>;
    fn try_from_slice(id: Option<u8>, body: Vec<u8>) -> Result<Self::Request, serde_json::Error>;
    // fn try_from_str(id: Option<u8>, body: &str) -> Result<Self::Request, serde_json::Error>;
    // fn to_string(&self) -> Result<String, serde_json::Error>;
    // fn item_id() -> u8;
    // fn to_bytes(&self) -> Vec<u8>;
}
#[typetag::serde(tag = "type")]
pub trait FileRequest: Send + Sync {
    fn as_any(&self) -> &dyn Any;
}
#[typetag::serde(tag = "type")]
pub trait FileResponse {
    // fn item_id() -> u8;
    fn as_any(&self) -> &dyn Any;
}

// pub trait FileResponseExecuatable {
//     type Request;
//     fn try_from_str(id: Option<u8>, body: &str) -> Result<Self::Request, serde_json::Error>;
//     fn item_id() -> u8;
//     fn set_state_id(&mut self, id: u8);
//     // fn to_string(&self) -> Result<String, serde_json::Error>;
// }

#[derive(Deserialize, Serialize)]
pub struct CannonolizeRequest {
    location: String,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct DirectoryResponse {
    id: u8,
    directory: Vec<FsItem>,
}
#[typetag::serde]
impl FileResponse for DirectoryResponse {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
// impl FileResponseExecuatable for DirectoryResponse {
//     fn item_id() -> u8 {
//         2
//     }
// }
// impl FileResponseExecuatable for DirectoryResponse {
//     type Request = Self;

//     fn try_from_str(id: Option<u8>, body: &str) -> Result<Self::Request, serde_json::Error> {
//         let mut request = serde_json::from_str::<Self>(body)?;
//         request.id = id.unwrap_or(0);
//         Ok(request)
//     }
//     fn item_id() -> u8 {
//         2
//     }
//     fn set_state_id(&mut self, id: u8) {
//         self.id = id;
//     }
//     // fn to_string(&self) -> Result<String, serde_json::Error> {
//     //     Ok(format!("0{}{}", self.id, serde_json::to_string(self)?))
//     // }
// }

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

#[typetag::serde]
impl FileRequest for LsRequest {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
#[derive(Deserialize, Serialize)]
pub struct FileTransferRequest {
    pub id: u8,
    pub bytes: Vec<u8>
}

impl FileRequestExecutable for LsRequest {
    type Request = Self;
    fn execute_bytes(&self) -> Result<Vec<u8>, FileOperationResult> {
        //println!("{:#?}", self.location);
        // let mut location = self.location.clone();
        // if !(location.starts_with("server") || location.starts_with("/server")){
        //     location = format!("./server/{}", location);
        // }
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
            println!("{:#?}", response);
            // let mut bytes = Vec::new();

            // bytes.extend(serde_json::to_vec(&response).unwrap());
            Ok(serde_json::to_vec(&response).unwrap())
        } else {
            println!("was not given a dir");
            return Err(FileOperationResult::InvalidOperation);
        }
    }
    fn execute_bytes_tagged(&self) -> Result<Vec<u8>, FileOperationResult> {
        let mut bytes = Vec::new();
        // bytes.push(DirectoryResponse::item_id());
        // bytes.push(self.id);
        bytes.extend(self.execute_bytes()?);
        Ok(bytes)
    }
    // fn try_from_str(id: Option<u8>, body: &str) -> Result<Self::Request, serde_json::Error> {
    //     let mut request = serde_json::from_str::<Self>(body)?;
    //     request.id = id.unwrap_or(0);
    //     Ok(request)
    // }
    fn try_from_slice(id: Option<u8>, body: Vec<u8>) -> Result<Self::Request, serde_json::Error> {
        println!("trying from slice");
        let mut request = serde_json::from_slice::<Self>(&body)?;
        request.id = id.unwrap_or(0);
        println!("passed it");
        Ok(request)
    }
    // fn to_string(&self) -> Result<String, serde_json::Error> {
    //     Ok(format!("0{}{}", self.id, serde_json::to_string(self)?))
    // }

    // fn item_id() -> u8 {
    //     1
    // }

    // fn to_bytes(&self) -> Vec<u8> {
    //     let mut bytes = Vec::new();
    //     // bytes.push(LsRequest::item_id());
    //     // bytes.push(self.id);
    //     bytes.extend(serde_json::to_vec(self).unwrap());
    //     bytes
    // }
}
// impl FileRequestExecutable for FileTransferRequest {
    
// }

#[derive(Deserialize, Serialize)]
pub enum FileRequestErrors {
    NoRequestHeader,
    CouldNotFindRequest,
    InvalidRequestHeader,
    InvalidBody,
    CouldNotParse,
}

