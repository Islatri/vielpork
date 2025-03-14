use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

use crate::base::enums::TaskState;
use crate::base::structs::DownloadProgress;
use crate::error::Result;

#[derive(Serialize, Deserialize)]
pub struct PersistentState {
    pub tasks: Vec<TaskStateRecord>,
}

#[derive(Serialize, Deserialize)]
pub struct TaskStateRecord {
    pub id: u32,
    pub url: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub file_path: PathBuf,
    pub state: TaskState,
}

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub id: u32,
    pub url: String,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    cancel_token: tokio_util::sync::CancellationToken,
    pub state: Arc<RwLock<TaskState>>,
    pub progress: Arc<Mutex<DownloadProgress>>,
    pub file_path: PathBuf,
    pub total_size: u64,
}

impl DownloadTask {
    pub fn new(id: u32, url: String, file_path: PathBuf, total_size: u64) -> Self {
        Self {
            id,
            url,
            handle: Arc::new(Mutex::new(None)),
            cancel_token: tokio_util::sync::CancellationToken::new(),
            state: Arc::new(RwLock::new(TaskState::default())),
            progress: Arc::new(Mutex::new(DownloadProgress {
                bytes_downloaded: 0,
                total_bytes: total_size,
                progress_percentage: 0.0,
                rate: 0.0,
                remaining_time: std::time::Duration::from_secs(0),
            })),
            file_path,
            total_size,
        }
    }
    pub async fn transition_state(&self, new_state: TaskState) -> Result<()> {
        let mut current = self.state.write().await;

        if *current == TaskState::Canceled {
            return Ok(());
        }

        let valid = match (*current, new_state) {
            (TaskState::Paused, TaskState::Downloading) => true,
            (TaskState::Paused, TaskState::Paused) => true,
            (TaskState::Paused, TaskState::Pending) => true,
            (TaskState::Pending, TaskState::Paused) => true,
            (TaskState::Pending, TaskState::Downloading) => true,
            (TaskState::Downloading, TaskState::Paused) => true,
            (TaskState::Downloading, TaskState::Completed) => true,
            (TaskState::Failed, _) => true,
            (_, TaskState::Failed) => true,
            (_, TaskState::Canceled) => true,
            _ => false,
        };

        if valid {
            *current = new_state;
            Ok(())
        } else {
            Err(format!("Cannot transition from {:?} to {:?}", *current, new_state).into())
        }
    }

    pub async fn start(&self) -> Result<()> {
        self.transition_state(TaskState::Downloading).await
    }

    pub async fn pause(&self) -> Result<()> {
        self.transition_state(TaskState::Paused).await
    }

    pub async fn resume(&self) -> Result<()> {
        self.transition_state(TaskState::Pending).await
    }

    pub async fn cancel(&self) -> Result<()> {
        self.cancel_token.cancel();
        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
        self.transition_state(TaskState::Canceled).await
    }
    pub fn task_id(&self) -> u32 {
        self.id
    }
}
