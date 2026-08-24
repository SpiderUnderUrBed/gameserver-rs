use std::error::Error;

use crate::{
    database::databasespec::{K8sType, NodeType}, kubernetes::{GetK8sTypeRequest, VerifyIsK8sGameserverRequest}, orchestrator::kubernetes::{
        BuildDeploymentRequest, GetK8sGameserversRequest, ListNodeInfoRequest,
    }, NodeWithStream, Status
};
use k8s_orchestrator::kubernetes::{
    create_k8s_deployment, get_avalible_gameserver, get_k8s_type, list_node_info,
    verify_is_k8s_gameserver,
};
use kube::Client;
use serde_json::Value;

#[derive(Clone)]
pub struct K8sLocalClient {
    pub k8s_client: Client,
    pub docker_info: String,
}

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
        get_avalible_gameserver(&connection.k8s_client)
            .await
            .map(|s| Some(s))
    }
}
impl KubeLocalRequest for BuildDeploymentRequest {
    type Output = ();

    async fn execute_locally(
        &self,
        connection: K8sLocalClient,
        // client: Client,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        create_k8s_deployment(&connection.k8s_client, self.deployment.clone()).await
    }
}
impl KubeLocalRequest for ListNodeInfoRequest {
    type Output = Vec<NodeWithStream>;

    async fn execute_locally(
        &self,
        connection: K8sLocalClient,
        // client: Client,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        match list_node_info(connection.k8s_client).await {
            Ok(nodes) => {
                let final_nodes: Vec<NodeWithStream> = nodes
                    .iter()
                    .map(|node| NodeWithStream {
                        name: node.name.clone(),
                        ip: node.ip.clone(),
                        status: Status::Unknown,
                        nodetype: NodeType::Unknown,
                        k8s_type: node.k8s_type.clone().into(),
                        gameserver: Value::String(node.gameserver.clone()),
                        tx: None,
                        rx: None,
                    })
                    .collect();
                Ok(final_nodes)
            }
            Err(e) => return Err(e),
        }
    }
}
impl KubeLocalRequest for VerifyIsK8sGameserverRequest {
    type Output = bool;

    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sLocalClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        verify_is_k8s_gameserver(connection.k8s_client, self.server.clone()).await
    }
}
impl KubeLocalRequest for GetK8sTypeRequest {
    type Output = K8sType;

    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sLocalClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        Ok(get_k8s_type(&connection.k8s_client, self.server.clone()).await?.into())
    }
}

impl Into<K8sType> for k8s_orchestrator::kubernetes::K8sType {
    fn into(self) -> K8sType { 
        match self {
            k8s_orchestrator::kubernetes::K8sType::Node => K8sType::Node,
            k8s_orchestrator::kubernetes::K8sType::Pod => K8sType::Pod,
            k8s_orchestrator::kubernetes::K8sType::None => K8sType::None,
            k8s_orchestrator::kubernetes::K8sType::Inbuilt => K8sType::Inbuilt,
            k8s_orchestrator::kubernetes::K8sType::Unknown => K8sType::Unknown,
        }
    }
}