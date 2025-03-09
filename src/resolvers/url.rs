
use crate::error::Result;
use crate::base::traits::ResourceResolver;
use crate::base::structs::{DownloadResource, ResolvedResource};
use async_trait::async_trait;

#[derive(Debug,Clone)]
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
            DownloadResource::Url(url) => {
                Ok(ResolvedResource{
                    url: url.clone(),
                    headers: vec![],
                    auth: None,
                })
            }
            DownloadResource::Resolved(resolved) => {
                Ok(resolved.clone())
            }
            _ => {
                Err("Unsupported resource type".into())
            }
        }
    }
}
