use crate::base::algorithms::generate_task_id;
use crate::base::enums::DownloadResource;
use crate::base::structs::ResolvedResource;
use crate::base::traits::ResourceResolver;
use crate::error::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct UrlResolver {}

impl UrlResolver {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ResourceResolver for UrlResolver {
    async fn resolve(&self, resource: &DownloadResource) -> Result<ResolvedResource> {
        match resource {
            DownloadResource::Url(url) => Ok(ResolvedResource {
                id: generate_task_id(url),
                url: url.clone(),
                headers: vec![],
                auth: None,
            }),
            DownloadResource::Resolved(resolved) => Ok(resolved.clone()),
            _ => Err("Unsupported resource type".into()),
        }
    }
}
