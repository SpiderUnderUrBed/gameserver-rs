
use std::error::Error;

// use k8s_orchestrator::kubernetes::K8sNode;
// use k8s_orchestrator::kubernetes::K8sType;
use tonic::transport::Channel;

use crate::database::databasespec::NodeType;
use crate::{orchestrator::{kubernetes::{GetK8sTypeRequest, BuildDeploymentRequest, GetK8sGameserversRequest, ListNodeInfoRequest}, docker::BuildImageRequest}, NodeWithStream};
use crate::{Status};
use crate::database::databasespec::K8sNode;
use crate::K8sType;

mod proto {
    tonic::include_proto!("kube");
}
use proto::{k8s_client::K8sClient, docker_client::DockerClient};


pub trait KubeRemoteRequest {
    type Output;
    async fn execute_remote(
        &self,
        // client: Client,
        connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

#[derive(Clone)]
pub struct K8sRemoteClient {
    kubernetes_client: K8sClient<Channel>,
    docker_client: DockerClient<Channel>
}
impl K8sRemoteClient {
    pub async fn connect(url: String) -> Result<K8sRemoteClient, Box<dyn Error + Send + Sync>> {
        let channel = Channel::from_shared(url.clone())?.connect().await?;
        let kubernetes_client: K8sClient<Channel> = K8sClient::new(channel.clone());
        let docker_client: DockerClient<Channel> = DockerClient::new(channel.clone());

        Ok(K8sRemoteClient {  
            kubernetes_client,
            docker_client
        })
    }
}
impl KubeRemoteRequest for BuildImageRequest {
    type Output = ();
    async fn execute_remote(
        &self,
        // client: Client,
        mut connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let request = proto::BuildImageRequest {};
        connection.docker_client.build_image(request).await?;
        Ok(())
    }
}

impl KubeRemoteRequest for BuildDeploymentRequest {
    type Output = ();
    async fn execute_remote(
        &self,
        // client: Client,
        mut connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let request = proto::BuildDepolymentRequest {
            deployment: self.deployment.clone(),
        };
        connection.kubernetes_client.build_deployment(request).await?;
        Ok(())
    }
}
impl KubeRemoteRequest for GetK8sGameserversRequest {
    type Output = Option<String>;
    async fn execute_remote(
        &self,
        // client: Client,
        mut connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let request = proto::GetGameserverRequest {};
        let response = connection.kubernetes_client.get_gameservers(request).await?;
        Ok(Some(response.get_ref().gameserver.clone()))
    }
}
impl KubeRemoteRequest for GetK8sTypeRequest {
    type Output = K8sType;
    async fn execute_remote(
        &self,
        // client: Client,
        mut connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>{
        let request = proto::GetK8sTypeRequest {
            server: self.server.clone()
        };
        let response = connection.kubernetes_client.get_k8s_type(request).await?;
        Ok(K8sType::try_from(response.get_ref().clone().kind)?)
    }
}
impl KubeRemoteRequest for ListNodeInfoRequest {
    type Output = Vec<NodeWithStream>;
    async fn execute_remote(
        &self,
        // client: Client,
        mut connection: K8sRemoteClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>{
        let request = proto::ListNodeInfoRequest {};
        let response = connection.kubernetes_client.list_node_info(request).await?;

        let nodes = response
            .get_ref()
            .k8s_nodes
            .clone()
            .into_iter()
            .map(K8sNode::try_from)
            .filter_map(|node_result| {
                    if let Ok(node) = node_result {
                        Some(NodeWithStream {
                            name: node.name,
                            ip: node.ip,
                            status: Status::Unknown,
                            nodetype: NodeType::Inbuilt,
                            k8s_type: node.k8s_type.into(),
                            gameserver: serde_json::Value::String(node.gameserver),
                            tx: None,
                            rx: None,
                        })
                    } else {
                        None
                    }
                }
            )
            .collect::<Vec<_>>();

        Ok(nodes)
    }
}
impl TryFrom<proto::K8sNode> for K8sNode {
    type Error = Box<dyn Error + Send + Sync>;

    fn try_from(node: proto::K8sNode) -> Result<Self, Self::Error> {
        Ok(K8sNode { name: node.name, ip: node.ip, gameserver: node.gameserver, k8s_type: 
            {
                if let Some(node_type) = node.k8s_type {
                    K8sType::try_from(node_type.kind)?
                } else {
                    return Err("cannot parse".into())
                }
            }
        })
    }
}