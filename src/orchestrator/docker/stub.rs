use std::error::Error;

use crate::K8sLocalClient;

use crate::{orchestrator::docker::BuildImageRequest};

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
        Err("not implimented".into())
    }
}

