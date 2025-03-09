use super::structs::{ResolvedResource, DownloadProgress};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum DownloaderState {
    #[default]
    Idle,      
    Running,   
    Suspended,    
    Stopped,   
}

#[derive(Debug, Clone, Copy, Eq,PartialEq, Default,Serialize, Deserialize)]
pub enum TaskState {
    #[default]
    Pending,     
    Downloading, 
    Paused,      
    Completed,   
    Canceled,
    Failed,      
}

#[derive(Clone)]
pub enum ProgressEvent { 
    Start { task_id: u32, total: u64 },
    Update { task_id: u32, progress: DownloadProgress },
    Finish { task_id: u32 ,finish: DownloadResult},
    OperationResult {
        operation: OperationType, code: u32, message: String
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DownloadResult {
    Success {
        path: std::path::PathBuf,
        size: u64,
        duration: std::time::Duration,
    },
    Failed {
        error: String,
        retryable: bool,
    },
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthMethod {
    None,
    BasicAuth { username: String, password: String },
    OAuth2 { token: String },
    ApiKey { key: String, header: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    // 下载结果
    Download,
    DownloadTask(u32),

    // 全局操作
    StartAll,
    PauseAll,
    ResumeAll,
    CancelAll,
    
    // 单任务操作
    StartTask(u32),
    PauseTask(u32),
    ResumeTask(u32),
    CancelTask(u32),
    
    // 系统级操作
    ChangeConcurrency(u32),
    SetRateLimit(u64),
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::Download => write!(f, "Download"),
            OperationType::DownloadTask(id) => write!(f, "Download task {}", id),
            OperationType::StartAll => write!(f, "Start all tasks"),
            OperationType::PauseAll => write!(f, "Pause all tasks"),
            OperationType::ResumeAll => write!(f, "Resume all tasks"),
            OperationType::CancelAll => write!(f, "Cancel all tasks"),
            OperationType::StartTask(id) => write!(f, "Start task {}", id),
            OperationType::PauseTask(id) => write!(f, "Pause task {}", id),
            OperationType::ResumeTask(id) => write!(f, "Resume task {}", id),
            OperationType::CancelTask(id) => write!(f, "Cancel task {}", id),
            OperationType::ChangeConcurrency(n) => write!(f, "Change concurrency to {}", n),
            OperationType::SetRateLimit(n) => write!(f, "Set rate limit to {} B/s", n),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileChecksum {
    MD5(String),
    SHA1(String),
    SHA256(String),
    Custom { algorithm: String, value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DownloadResource {
    Url(String),
    Id(String), 
    Params(Vec<String>),
    Resolved(ResolvedResource),
}
