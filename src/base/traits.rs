
use crate::error::Result;
use super::structs::{DownloadProgress,  ResolvedResource};
use super::enums::{DownloadResult,DownloadResource, OperationType};
use async_trait::async_trait;

#[async_trait]
pub trait ProgressReporter
{
    async fn start_task(&self, task_id: u32, total: u64) -> Result<()>;
    async fn update_progress(&self, task_id: u32, progress: &DownloadProgress) -> Result<()>;
    async fn finish_task(&self, task_id: u32, result: DownloadResult) -> Result<()>;
}

#[async_trait]
pub trait ResultReporter
{
    async fn operation_result(&self, operation: OperationType, code: u32, message: String) -> Result<()>;
}

// 异步trait的动态分发是肯定要解决的问题，这里可能真就只能暂时手写Box试试了

pub trait CombinedReporter: ProgressReporter + ResultReporter + Send + Sync {}
impl<T: ProgressReporter + ResultReporter + Send + Sync> CombinedReporter for T {}

// `(dyn CombinedReporter + 'static)` doesn't implement `Debug`
// the trait `Debug` is not implemented for `(dyn CombinedReporter + 'static)`
// the following other types implement trait `Debug`:
//   dyn std::any::Any + std::marker::Send + Sync
//   dyn std::any::Any + std::marker::Send
//   dyn std::any::Any
//   dyn tracing_core::field::Value

// 为什么这个trait对象不能实现Debug呢？因为trait对象是动态分发的，所以大小是不确定的，所以不能实现Debug

#[async_trait]
pub trait ResourceResolver: Send + Sync {
    async fn resolve(&self, resource: &DownloadResource) -> Result<ResolvedResource>;
}