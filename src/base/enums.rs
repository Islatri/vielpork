use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum DownloaderState {
    #[default]
    Idle,      
    Running,   
    Suspended,    
    Stopped,   
}

#[derive(Debug, Clone, Copy, PartialEq, Default,Serialize, Deserialize)]
pub enum TaskState {
    #[default]
    Pending,     
    Downloading, 
    Paused,      
    Completed,   
    Canceled,
    Failed,      
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FinishType {
    Success,
    Failed,
    Canceled,
}
