use std::error::Error;
use std::fs;

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Node;
use k8s_openapi::api::core::v1::{PersistentVolume, PersistentVolumeClaim, Service};
use kube::api::PostParams;
use kube::Error::Api as ErrorApi;
use kube::{Api, Client};

//
pub async fn list_node_names(client: Client) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let nodes: Api<Node> = Api::all(client);
    let node_list = nodes.list(&Default::default()).await?;
    let names = node_list
        .items
        .into_iter()
        .filter_map(|node| node.metadata.name)
        .collect();
    Ok(names)
}
pub async fn create_k8s_deployment(
    client: &Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //let testing_deployment = !std::env::var("TESTING_DEPLOYMENT").unwrap_or("").is_empty();
    let deployment = if std::env::var("TESTING").is_ok() {
        println!("Using dev deployment");
        "deployment-dev.yaml"
    } else {
        "deployment.yaml"
    };

    let deployment_yaml =
        fs::read_to_string(format!("/usr/src/app/src/gameserver/{}", deployment))?;

    for doc in deployment_yaml.split("---") {
        let trimmed = doc.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(deployment) = serde_yaml::from_str::<Deployment>(trimmed) {
            let api: Api<Deployment> = Api::namespaced(client.clone(), "default");
            match api.create(&PostParams::default(), &deployment).await {
                Ok(_) => println!("Deployment created successfully!"),
                Err(ErrorApi(err)) if err.code == 409 => {
                    println!("Deployment already exists, skipping...");
                }
                Err(e) => return Err(Box::new(e)),
            }
        } else if let Ok(service) = serde_yaml::from_str::<Service>(trimmed) {
            let api: Api<Service> = Api::namespaced(client.clone(), "default");
            match api.create(&PostParams::default(), &service).await {
                Ok(_) => println!("Service created successfully!"),
                Err(ErrorApi(err)) if err.code == 409 => {
                    println!("Service already exists, skipping...");
                }
                Err(e) => return Err(Box::new(e)),
            }
        } else if let Ok(pv) = serde_yaml::from_str::<PersistentVolume>(trimmed) {
            let api: Api<PersistentVolume> = Api::all(client.clone());
            match api.create(&PostParams::default(), &pv).await {
                Ok(_) => println!("PersistentVolume created successfully!"),
                Err(ErrorApi(err)) if err.code == 409 => {
                    println!("PersistentVolume already exists, skipping...");
                }
                Err(e) => return Err(Box::new(e)),
            }
        } else if let Ok(pvc) = serde_yaml::from_str::<PersistentVolumeClaim>(trimmed) {
            let api: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), "default");
            match api.create(&PostParams::default(), &pvc).await {
                Ok(_) => println!("PersistentVolumeClaim created successfully!"),
                Err(ErrorApi(err)) if err.code == 409 => {
                    println!("PersistentVolumeClaim already exists, skipping...");
                }
                Err(e) => return Err(Box::new(e)),
            }
        } else {
            eprintln!("Unknown or unsupported YAML document:\n{}", trimmed);
        }
    }

    Ok(())
}
