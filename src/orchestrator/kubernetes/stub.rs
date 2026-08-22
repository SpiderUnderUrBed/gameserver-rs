
use std::error::Error;


use crate::{orchestrator::kubernetes::{BuildDeploymentRequest, GetK8sGameserversRequest, ListNodeInfoRequest}, K8sClient, NodeWithStream};


pub trait KubeLocalRequest {
    type Output;
    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}
impl KubeLocalRequest for GetK8sGameserversRequest {
    type Output = Option<String>;

    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("not enabled".into())
    }
}
impl KubeLocalRequest for BuildDeploymentRequest {
    type Output = ();

    async fn execute_locally(
        &self,
        connection: K8sClient,
        // client: Client,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("not enabled".into())
    }
}
impl KubeLocalRequest for ListNodeInfoRequest {
    type Output = Vec<NodeWithStream>;

    async fn execute_locally(
        &self,
        connection: K8sClient,
        // client: Client,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("not enabled".into())
    }
}

