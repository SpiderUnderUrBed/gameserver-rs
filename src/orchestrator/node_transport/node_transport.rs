use std::error::Error;

use k8s_orchestrator::kubernetes::K8sType;

use crate::{orchestrator::{docker::BuildImageRequest, kubernetes::{BuildDeploymentRequest, GetK8sGameserversRequest, GetK8sTypeRequest, ListNodeInfoRequest}}, NodeWithStream};

#[derive(Clone)]
pub struct K8sRemoteClient {

}
impl K8sRemoteClient {
    pub async fn connect(_url: String) -> Result<K8sRemoteClient, Box<dyn Error + Send + Sync>>{
        Ok(K8sRemoteClient {})
    }
}

pub trait KubeRemoteRequest {
    type Output;
    async fn execute_remote(
        &self,
        // client: Client,
        connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}
impl KubeRemoteRequest for GetK8sTypeRequest {
    type Output = K8sType;

    async fn execute_remote(
        &self,
        // client: Client,
        connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("unimplimented".into())
    }
}
impl KubeRemoteRequest for BuildDeploymentRequest {
    type Output = ();

    async fn execute_remote(
        &self,
        // client: Client,
        connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("unimplimented".into())
    }
}
impl KubeRemoteRequest for GetK8sGameserversRequest {
    type Output = Option<String>;

    async fn execute_remote(
        &self,
        // client: Client,
        connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("unimplimented".into())
    }
}
impl KubeRemoteRequest for ListNodeInfoRequest {
    type Output = Vec<NodeWithStream>;

    async fn execute_remote(
        &self,
        // client: Client,
        connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("unimplimented".into())
    }
}
impl KubeRemoteRequest for BuildImageRequest {
    type Output = ();

    async fn execute_remote(
        &self,
        // client: Client,
        connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("unimplimented".into())
    }
}