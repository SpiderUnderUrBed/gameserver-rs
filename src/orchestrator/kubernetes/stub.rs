use std::error::Error;

use crate::{
    K8sLocalClient, NodeWithStream,
    database::databasespec::K8sType,
    kubernetes::{GetK8sTypeRequest, VerifyIsK8sGameserverRequest},
    orchestrator::kubernetes::{
        BuildDeploymentRequest, GetK8sGameserversRequest, ListNodeInfoRequest,
    },
};

pub trait KubeLocalRequest {
    type Output;
    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sLocalClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

impl KubeLocalRequest for GetK8sGameserversRequest {
    type Output = Option<String>;

    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sLocalClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("not enabled".into())
    }
}
impl KubeLocalRequest for BuildDeploymentRequest {
    type Output = ();

    async fn execute_locally(
        &self,
        connection: K8sLocalClient,
        // client: Client,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("not enabled".into())
    }
}
impl KubeLocalRequest for ListNodeInfoRequest {
    type Output = Vec<NodeWithStream>;

    async fn execute_locally(
        &self,
        connection: K8sLocalClient,
        // client: Client,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("not enabled".into())
    }
}
impl KubeLocalRequest for VerifyIsK8sGameserverRequest {
    type Output = bool;

    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sLocalClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("not enabled".into())
    }
}
impl KubeLocalRequest for GetK8sTypeRequest {
    type Output = K8sType;

    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sLocalClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Err("not enabled".into())
    }
}
