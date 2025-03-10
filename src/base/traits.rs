use super::enums::{DownloadResource, DownloadResult, OperationType};
use super::structs::{DownloadProgress, ResolvedResource};
use crate::error::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ProgressReporter {
    async fn start_task(&self, task_id: u32, total: u64) -> Result<()>;
    async fn update_progress(&self, task_id: u32, progress: &DownloadProgress) -> Result<()>;
    async fn finish_task(&self, task_id: u32, result: DownloadResult) -> Result<()>;
}

#[async_trait]
pub trait ResultReporter {
    async fn operation_result(
        &self,
        operation: OperationType,
        code: u32,
        message: String,
    ) -> Result<()>;
}

pub trait CombinedReporter: ProgressReporter + ResultReporter + Send + Sync {}
impl<T: ProgressReporter + ResultReporter + Send + Sync> CombinedReporter for T {}

#[async_trait]
pub trait ResourceResolver: Send + Sync {
    async fn resolve(&self, resource: &DownloadResource) -> Result<ResolvedResource>;
}
