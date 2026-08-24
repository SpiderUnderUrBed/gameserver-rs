use std::error::Error;

use k8s_orchestrator::docker::build_docker_image;

use crate::{kubernetes::local::K8sLocalClient, orchestrator::docker::BuildImageRequest};

pub trait DockerLocalRequest {
    type Output;
    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sLocalClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

impl DockerLocalRequest for BuildImageRequest {
    type Output = ();

    async fn execute_locally(
        &self,
        // client: Client,
        connection: K8sLocalClient,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        build_docker_image().await
    }
}
