use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub rate: f64,
    pub remaining_time: std::time::Duration,
    pub progress_percentage: f64,
}

// pub struct DownloadProgress {
//     /// 已下载字节数
//     pub bytes_downloaded: u64,
//     /// 总字节数（可能未知）
//     pub total_bytes: Option<u64>,
//     /// 实时下载速率（字节/秒）
//     pub current_rate: f64,
//     /// 平均下载速率（字节/秒）
//     pub average_rate: f64,
//     /// 已用时间
//     pub elapsed_time: std::time::Duration,
//     /// 预估剩余时间（当总大小未知时为None）
//     pub remaining_time: Option<std::time::Duration>,
//     /// 下载进度百分比（0.0-100.0）
//     pub progress_percentage: Option<f64>,
// }



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadOptions {
    // 基础配置
    /// 保存路径（目录或完整文件路径）
    pub save_path: String,
    /// 是否自动创建目录
    pub create_dirs: bool,
    
    // 网络配置
    /// 自定义HTTP头
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// 用户代理
    pub user_agent: Option<String>,
    /// 请求超时（秒）
    pub timeout: u64,
    /// 最大重定向次数
    pub max_redirects: u32,
    /// 是否验证TLS证书
    pub tls_verify: bool,
    
    // 并发控制
    /// 最大并发连接数
    pub concurrency: u32,
    /// 分块下载大小（None表示自动检测）
    pub chunk_size: Option<u64>,
    /// 是否启用范围请求
    pub enable_range: bool,
    
    // 流量控制
    /// 全局速率限制（字节/秒）
    pub rate_limit: Option<u64>,
    /// 每个连接的速率限制（字节/秒）
    pub per_connection_rate_limit: Option<u64>,
    
    // 高级功能
    /// 最大重试次数
    pub max_retries: u32,
    /// 代理设置（支持http/https/socks5）
    pub proxy: Option<String>,
    /// 是否启用断点续传
    pub resume_download: bool,
    /// 下载缓冲区大小（字节）
    pub buffer_size: usize,
    /// 进度更新频率（毫秒）
    pub progress_interval: u64,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            save_path: "downloads".to_string(),
            create_dirs: true,
            headers: Vec::new(),
            user_agent: Some("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36".into()),
            timeout: 30,
            max_redirects: 5,
            tls_verify: true,
            concurrency: 4,
            chunk_size: None,
            enable_range: true,
            rate_limit: None,
            per_connection_rate_limit: None,
            max_retries: 3,
            proxy: None,
            resume_download: false,
            buffer_size: 8192,  // 8KB
            progress_interval: 500,
        }
    }
}

// Builder实现（示例部分方法）
impl DownloadOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_save_path(mut self, path: impl Into<String>) -> Self {
        self.save_path = path.into();
        self
    }

    pub fn with_concurrency(mut self, n: u32) -> Self {
        self.concurrency = n;
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    pub fn with_proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    pub fn with_rate_limit(mut self, limit: u64) -> Self {
        self.rate_limit = Some(limit);
        self
    }

    pub fn with_per_connection_rate_limit(mut self, limit: u64) -> Self {
        self.per_connection_rate_limit = Some(limit);
        self
    }

    pub fn with_resume_download(mut self, resume: bool) -> Self {
        self.resume_download = resume;
        self
    }

}