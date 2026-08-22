
#[cfg(feature = "full-stack")]
#[path = "local.rs"]
pub mod local;

#[cfg(not(feature = "full-stack"))]
#[path = "stub.rs"]
pub mod local;

pub struct GetK8sGameserversRequest {
    // pub connection: K8sClient
}

pub struct VerifyIsK8sGameserverRequest {
    pub server: String
}

pub struct BuildDeploymentRequest {
    // pub connection: K8sClient,
    pub deployment: String
}

pub struct ListNodeInfoRequest {
    // pub connection: K8sClient,
}
pub struct GetK8sTypeRequest {
    pub server: String
}