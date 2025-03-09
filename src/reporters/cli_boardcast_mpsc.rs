use crate::error::Result;
use crate::base::traits::{ProgressReporter, ResultReporter};
use crate::base::structs::DownloadProgress;
use crate::base::enums::{ProgressEvent, DownloadResult, OperationType};
use async_trait::async_trait;

#[derive(Debug,Clone)]
pub struct CliReporterBoardcastMpsc{
    inner_tx: tokio::sync::broadcast::Sender<ProgressEvent>,
    buffer_size: usize,
}
impl CliReporterBoardcastMpsc {
    pub fn new(buffer_size: usize) -> Self {
        let (inner_tx, _) = tokio::sync::broadcast::channel(buffer_size);
        Self { inner_tx, buffer_size }
    }

    // 将 broadcast 接收端转换为 mpsc 接收端
    pub fn subscribe_mpsc(&self) -> tokio::sync::mpsc::Receiver<ProgressEvent> {
        let (tx, rx) = tokio::sync::mpsc::channel(self.buffer_size);
        let mut inner_rx = self.inner_tx.subscribe();
        
        // 启动转发任务
        tokio::spawn(async move {
            // while let Ok(event) = inner_rx.recv().await {
            //     if tx.send(event).await.is_err() {
            //         break;
            //     }
            // }
            loop {
                match inner_rx.recv().await {
                    Ok(event) => {
                        if tx.send(event).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        // 可以选择重试或等待一段时间
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        
        rx
    }

    // 发送事件的方法
    pub async fn send(&self, event: ProgressEvent) -> Result<usize> {
        self.inner_tx.send(event)?;
        Ok(self.inner_tx.receiver_count())
    }

    // 创建新订阅者
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ProgressEvent> {
        self.inner_tx.subscribe()
    }
}

#[async_trait]
impl ProgressReporter for CliReporterBoardcastMpsc {
    async fn start_task(&self, task_id: u32, total: u64) ->Result<()> {
        self.send(ProgressEvent::Start { task_id, total }).await?;
        Ok(())
    }

    async fn update_progress(&self, task_id: u32, progress: &DownloadProgress)->Result<()> {
        self.send(ProgressEvent::Update { task_id, progress: progress.clone() }).await?;
        Ok(())
    }

    async fn finish_task(&self, task_id: u32,finish: DownloadResult) ->Result<()>{
        self.send(ProgressEvent::Finish { task_id ,finish}).await?;
        Ok(())
    }
}

#[async_trait]
impl ResultReporter for CliReporterBoardcastMpsc {
    async fn operation_result(&self, operation: OperationType, code: u32, message: String) ->Result<()> {
        self.send(ProgressEvent::OperationResult { operation, code, message }).await?;
        Ok(())
    }
}
