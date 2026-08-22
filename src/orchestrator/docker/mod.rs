
#[cfg(feature = "full-stack")]
#[path = "local.rs"]
pub mod local;

#[cfg(not(feature = "full-stack"))]
#[path = "stub.rs"]
pub mod local;

pub struct BuildImageRequest {
    // pub(crate) client: K8sClient
}