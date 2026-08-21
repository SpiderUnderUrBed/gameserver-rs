
use std::error::Error;


use crate::{database::databasespec::NodeType, K8sClient, NodeWithStream, Status};
use k8s_orchestrator::kubernetes::{create_k8s_deployment, get_avalible_gameserver, list_node_info, list_node_names};
use serde_json::Value;

pub trait KubeLocalRequest {
    type Output;
    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

pub struct GetK8sGameserversRequest {
    // pub connection: K8sClient
}
impl KubeLocalRequest for GetK8sGameserversRequest {
    type Output = Option<String>;

    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        get_avalible_gameserver(&connection.k8s_client).await.map(|s| Some(s))
    }
}
pub struct BuildDeploymentRequest {
    // pub connection: K8sClient,
    pub deployment: String
}
impl KubeLocalRequest for BuildDeploymentRequest {
    type Output = ();

    async fn execute_locally(
        &self,
        connection: K8sClient,
        // client: Client,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        create_k8s_deployment(&connection.k8s_client, self.deployment.clone()).await
    }
}
pub struct ListNodeInfoRequest {
    // pub connection: K8sClient,
}
impl KubeLocalRequest for ListNodeInfoRequest {
    type Output = Vec<NodeWithStream>;

    async fn execute_locally(
        &self,
        connection: K8sClient,
        // client: Client,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        // list_node_info(connection).await
        //     .map(|nodes| nodes.into_iter().map(|node| {
        //         NodeWithStream {
        //             name: todo!(),
        //             ip: todo!(),
        //             status: todo!(),
        //             nodetype: todo!(),
        //             k8s_type: todo!(),
        //             gameserver: todo!(),
        //             tx: todo!(),
        //             rx: todo!(),
        //         }
        //     }).collect()).co
        match list_node_info(connection.k8s_client).await {
            Ok(nodes) => {
                let final_nodes: Vec<NodeWithStream> = nodes.iter().map(
                |node| 
                    NodeWithStream {
                        name: node.name.clone(),
                        ip: node.ip.clone(),
                        status: Status::Unknown,
                        nodetype: NodeType::Unknown,
                        k8s_type: node.k8s_type.clone().into(),
                        gameserver: Value::String(node.gameserver.clone()),
                        tx: None,
                        rx: None,
                    }
                ).collect();
                Ok(final_nodes)
            },
            Err(e) => return Err(e),
        }

    }
}

// use std::error::Error;
// use std::fs;

// use k8s_openapi::api::apps::v1::Deployment;
// use k8s_openapi::api::core::v1::Node;
// use k8s_openapi::api::core::v1::Pod;
// use k8s_openapi::api::core::v1::{PersistentVolume, PersistentVolumeClaim, Service};
// use serde_json::Value;

// use kube::Error::Api as ErrorApi;
// use kube::api::ListParams;
// use kube::api::PostParams;
// use kube::{Api, Client};

// use crate::K8sType;
// use crate::NodeWithStream;
// use crate::NodeType;
// use crate::Status;

// pub async fn list_node_info(client: Client) -> Result<Vec<NodeWithStream>, Box<dyn Error>> {
//     let nodes: Api<Node> = Api::all(client);
//     let node_list = nodes.list(&Default::default()).await?;

//     let mut result = Vec::new();

//     for node in node_list.items {
//         if let Some(name) = node.metadata.name {
//             if let Some(status) = node.status {
//                 if let Some(addresses) = status.addresses {
//                     let mut ip = None;
//                     for addr in &addresses {
//                         if addr.type_ == "InternalIP" {
//                             ip = Some(addr.address.clone());
//                             break;
//                         }
//                     }
//                     if ip.is_none() {
//                         for addr in &addresses {
//                             if addr.type_ == "ExternalIP" {
//                                 ip = Some(addr.address.clone());
//                                 break;
//                             }
//                         }
//                     }

//                     if let Some(ip) = ip {
//                         let nodetype = node
//                             .metadata
//                             .labels
//                             .as_ref()
//                             .and_then(|labels| labels.get("kubernetes.io/role").cloned())
//                             .unwrap_or_else(|| "unknown".to_string());

//                         result.push(NodeWithStream {
//                             name,
//                             ip,
//                             gameserver: Value::String(String::new()),
//                             status: Status::Unknown,
//                             nodetype: NodeType::InbuiltWithString(nodetype),
//                             tx: None,
//                             rx: None,
//                             k8s_type: K8sType::Unknown,
//                         });
//                     }
//                 }
//             }
//         }
//     }

//     Ok(result)
// }

// pub async fn verify_is_k8s_node(
//     client: &Client,
//     ip: String,
// ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
//     let nodes: Api<Node> = Api::all(client.clone());
//     let node_list = nodes.list(&Default::default()).await?;

//     for node in node_list.items {
//         if let Some(status) = node.status {
//             if let Some(addresses) = status.addresses {
//                 for addr in addresses {
//                     if (addr.type_ == "InternalIP" || addr.type_ == "ExternalIP")
//                         && addr.address == ip
//                     {
//                         return Ok(true);
//                     }
//                 }
//             }
//         }
//     }

//     Ok(false)
// }

// pub async fn verify_is_k8s_pod(
//     client: &Client,
//     ip: String,
// ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
//     let pods: Api<Pod> = Api::all(client.clone());
//     let pod_list = pods.list(&Default::default()).await?;

//     for pod in pod_list.items {
//         if let Some(status) = pod.status {
//             if let Some(pod_ip) = status.pod_ip {
//                 if pod_ip == ip {
//                     return Ok(true);
//                 }
//             }
//         }
//     }

//     Ok(false)
// }

// pub async fn list_node_names(client: Client) -> Result<Vec<String>, Box<dyn std::error::Error>> {
//     let nodes: Api<Node> = Api::all(client);
//     let node_list = nodes.list(&Default::default()).await?;
//     let names = node_list
//         .items
//         .into_iter()
//         .filter_map(|node| node.metadata.name)
//         .collect();
//     Ok(names)
// }

// pub async fn get_avalible_gameserver(
//     client: &Client,
// ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
//     let services: Api<Service> = Api::namespaced(client.clone(), "default");
//     let lp = ListParams::default();
//     let svc_list = services.list(&lp).await?;

//     for svc in svc_list.items {
//         if let Some(name) = &svc.metadata.name {
//             if name.contains("gameserver") && !name.contains("gameserver-postgres") {
//                 let dns_name = format!("{}.default.svc.cluster.local:8080", name);
//                 println!("Using gameserver service DNS: {}", dns_name);
//                 return Ok(dns_name);
//             }
//         }
//     }

//     Err("No gameserver service found".into())
// }

// pub async fn verify_is_k8s_gameserver(
//     _: crate::Client,
//     _: String,
// ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
//     Ok(true)
// }

// pub async fn create_k8s_deployment(
//     client: &Client,
// ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//     let deployment = if std::env::var("TESTING").is_ok() {
//         println!("Using dev deployment");
//         "deployment-dev.yaml"
//     } else {
//         "deployment.yaml"
//     };

//     let deployment_yaml = fs::read_to_string(format!("/usr/src/app/gameserver/{}", deployment))?;

//     for doc in deployment_yaml.split("---") {
//         let trimmed = doc.trim();
//         if trimmed.is_empty() {
//             continue;
//         }

//         if let Ok(deployment) = serde_yaml::from_str::<Deployment>(trimmed) {
//             let api: Api<Deployment> = Api::namespaced(client.clone(), "default");
//             match api.create(&PostParams::default(), &deployment).await {
//                 Ok(_) => println!("Deployment created successfully!"),
//                 Err(ErrorApi(err)) if err.code == 409 => {
//                     println!("Deployment already exists, skipping...");
//                 }
//                 Err(e) => return Err(Box::new(e)),
//             }
//         } else if let Ok(service) = serde_yaml::from_str::<Service>(trimmed) {
//             let api: Api<Service> = Api::namespaced(client.clone(), "default");
//             match api.create(&PostParams::default(), &service).await {
//                 Ok(_) => println!("Service created successfully!"),
//                 Err(ErrorApi(err)) if err.code == 409 => {
//                     println!("Service already exists, skipping...");
//                 }
//                 Err(e) => return Err(Box::new(e)),
//             }
//         } else if let Ok(pv) = serde_yaml::from_str::<PersistentVolume>(trimmed) {
//             let api: Api<PersistentVolume> = Api::all(client.clone());
//             match api.create(&PostParams::default(), &pv).await {
//                 Ok(_) => println!("PersistentVolume created successfully!"),
//                 Err(ErrorApi(err)) if err.code == 409 => {
//                     println!("PersistentVolume already exists, skipping...");
//                 }
//                 Err(e) => return Err(Box::new(e)),
//             }
//         } else if let Ok(pvc) = serde_yaml::from_str::<PersistentVolumeClaim>(trimmed) {
//             let api: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), "default");
//             match api.create(&PostParams::default(), &pvc).await {
//                 Ok(_) => println!("PersistentVolumeClaim created successfully!"),
//                 Err(ErrorApi(err)) if err.code == 409 => {
//                     println!("PersistentVolumeClaim already exists, skipping...");
//                 }
//                 Err(e) => return Err(Box::new(e)),
//             }
//         } else {
//             eprintln!("Unknown or unsupported YAML document:\n{}", trimmed);
//         }
//     }

//     Ok(())
// }
