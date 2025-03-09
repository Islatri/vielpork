use crate::task::{ DownloadTask, PersistentState, TaskStateRecord };
use crate::base::traits::{ CombinedReporter, ResourceResolver };
use crate::base::algorithms::rate_remaining_progress;
use crate::base::enums::{
    DownloadResult,
    DownloaderState,
    DownloadResource,
    OperationType,
    TaskState,
};
use crate::base::structs::{ DownloadProgress, DownloadOptions, ResolvedResource, DownloadMeta };
use crate::error::Result;
use crate::base::algorithms::{
    generate_task_id,
    auto_filename,
    custom_filename,
    organize_by_domain,
    organize_by_type,
    custom_directory,
};
use crate::template::{ TemplateRenderer, TemplateContext };
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::io::AsyncWriteExt;
use futures::stream::StreamExt;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    options: Arc<RwLock<DownloadOptions>>,
    state: Arc<RwLock<DownloaderState>>,
    tasks: Arc<RwLock<Vec<DownloadTask>>>,
    resolver: Arc<Box<dyn ResourceResolver>>,
    reporter: Arc<Box<dyn CombinedReporter>>,
    state_notifier: tokio::sync::broadcast::Sender<DownloaderState>,
}

impl Downloader {
    pub fn new(
        options: DownloadOptions,
        resolver: Box<dyn ResourceResolver>,
        reporter: Box<dyn CombinedReporter>
    ) -> Self {
        let client = reqwest::ClientBuilder
            ::new()
            .user_agent("osynicite")
            .default_headers(reqwest::header::HeaderMap::new())
            .build()
            .unwrap();
        Self {
            client,
            options: Arc::new(RwLock::new(options)),
            state: Arc::new(RwLock::new(DownloaderState::default())),
            tasks: Arc::new(RwLock::new(Vec::new())),
            resolver: Arc::new(resolver),
            reporter: Arc::new(reporter),
            state_notifier: tokio::sync::broadcast::channel(128).0,
        }
    }

    pub async fn transition_state(&self, new_state: DownloaderState) -> Result<()> {
        let mut current = self.state.write().await;

        // 定义合法状态转换
        let valid = match (*current, new_state) {
            (DownloaderState::Idle, DownloaderState::Running) => true,
            (DownloaderState::Running, DownloaderState::Suspended) => true,
            (DownloaderState::Suspended, DownloaderState::Running) => true,
            (DownloaderState::Running, DownloaderState::Stopped) => true,
            (DownloaderState::Suspended, DownloaderState::Stopped) => true,
            _ => false,
        };

        if valid {
            *current = new_state;
            self.state_notifier.send(new_state.clone()).ok();
            Ok(())
        } else {
            Err(format!("Cannot transition from {:?} to {:?}", *current, new_state).into())
        }
    }
    pub async fn update_options(&self, options: DownloadOptions) -> Self {
        *self.options.write().await = options;
        self.clone()
    }
    pub async fn get_options(&self) -> DownloadOptions {
        self.options.read().await.clone()
    }
    pub async fn get_tasks(&self) -> Vec<DownloadTask> {
        self.tasks.read().await.clone()
    }
    //
    pub async fn optimize_resources(
        &self,
        resources: Vec<DownloadResource>,
        state: PersistentState
    ) -> Vec<DownloadResource> {
        let mut optimized = Vec::new();
        let resolver = self.resolver.clone();
        // 先解析所有的资源，然后对比resolved_resources的url是否在state中且已完成或取消
        let resolved_resources = futures::future::join_all(
            resources.into_iter().map(async |resource| { resolver.resolve(&resource).await })
        ).await;

        for resolved in resolved_resources.iter() {
            match resolved {
                Ok(resolved) => {
                    let task = state.tasks.iter().find(|t| t.url == resolved.url);
                    match task {
                        Some(t) => {
                            if t.state != TaskState::Completed || t.state != TaskState::Canceled {
                                optimized.push(DownloadResource::Resolved(resolved.clone()));
                            }
                        }
                        None => {
                            optimized.push(DownloadResource::Resolved(resolved.clone()));
                        }
                    }
                }
                Err(_) => {}
            }
        }

        optimized
    }
    pub async fn start(&self, resources: Vec<DownloadResource>) -> Result<()> {
        // 这个解决重复下载...，必须要能够有id或者url解析之后的对应，否则就只能用url了
        // task记录url，然后这里解析和去准备一下，除掉对应url的resource
        let options = self.get_options().await;
        let save_path = &options.save_path;
        let state_path = PathBuf::from(save_path).join("downloading.json");
        let optimized_resources: Vec<DownloadResource>;
        if state_path.exists() {
            let contents = tokio::fs::read_to_string(&state_path).await?;
            let state: PersistentState = serde_json::from_str(&contents)?;
            optimized_resources = self.optimize_resources(resources, state).await;
        } else {
            optimized_resources = resources;
        }

        self.transition_state(DownloaderState::Running).await?;

        let downloader = self.clone();
        println!("Downloading {} resources", optimized_resources.len());

        let reporter = self.reporter.clone();
        tokio::spawn(async move {
            if let Err(e) = downloader.download_multi(optimized_resources).await {
                reporter
                    .operation_result(
                        OperationType::Download,
                        500,
                        format!("Download failed: {}", e)
                    ).await
                    .ok();
            }
        });
        Ok(())
    }

    pub async fn pause(&self) -> Result<()> {
        self.transition_state(DownloaderState::Suspended).await
    }
    pub async fn resume(&self) -> Result<()> {
        self.transition_state(DownloaderState::Running).await
    }
    pub async fn stop(&self) -> Result<()> {
        self.transition_state(DownloaderState::Stopped).await
    }

    async fn generate_path(
        &self,
        resource: &DownloadResource,
        resolved: &ResolvedResource,
        meta: &DownloadMeta
    ) -> Result<PathBuf> {
        let options = self.get_options().await;
        // 获取基础保存目录
        let base_dir = PathBuf::from(&options.save_path);

        // println!("options.path_policy {:?}", options.path_policy);

        let template = options.path_policy.template.as_deref().unwrap_or("");
        let dir_template = options.path_policy.dir_template.as_deref().unwrap_or("");
        let max_length = options.path_policy.max_length.unwrap_or(255);

        // 步骤1：确定文件名
        let filename = match options.path_policy.naming.as_str() {
            "auto" => auto_filename(resolved, meta).await?,
            "custom" =>
                custom_filename(
                    resource,
                    resolved,
                    &TemplateRenderer::new(),
                    meta,
                    template,
                    max_length
                ).await?,
            _ => {
                return Err("Invalid naming policy".into());
            }
        };

        // 步骤2：确定目录结构
        let subdir = match options.path_policy.organization.as_str() {
            "flat" => PathBuf::new(),
            "by_type" => organize_by_type(meta).await?,
            "by_domain" => organize_by_domain(resolved).await?,
            "custom" => {
                let path_buf = PathBuf::from(&filename);
                let extension = path_buf.extension().map(|e| e.to_str().unwrap_or_default());

                let context = TemplateContext {
                    url: &resolved.url,
                    domain: None,
                    filename: &filename,
                    extension,
                    meta,
                    download_time: chrono::Utc::now(),
                    custom_data: None,
                };
                custom_directory(dir_template, &context, &TemplateRenderer::new()).await?
            }
            _ => {
                return Err("Invalid organization policy".into());
            }
        };

        // 步骤3：构建完整路径
        let mut full_path = base_dir.join(subdir).join(filename);

        // 步骤4：处理路径冲突
        full_path = self.handle_conflict(full_path).await?;

        Ok(full_path)
    }

    async fn handle_conflict(&self, mut path: PathBuf) -> Result<PathBuf> {
        let mut counter = 1;
        let original_path = path.clone();
        let options = self.get_options().await;

        while path.exists() {
            match options.path_policy.conflict.as_str() {
                "overwrite" => {
                    break;
                }
                "rename" => {
                    let stem = original_path
                        .file_stem()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default();
                    let ext = original_path
                        .extension()
                        .map(|e| format!(".{}", e.to_str().unwrap_or_default()))
                        .unwrap_or_default();

                    path.set_file_name(format!("{}_{}{}", stem, counter, ext));
                    counter += 1;
                }
                "error" => {
                    return Err("File already exists".into());
                }
                _ => {
                    return Err("Invalid conflict policy".into());
                }
            }
        }
        Ok(path)
    }
    pub async fn download_multi(&self, resources: Vec<DownloadResource>) -> Result<()> {
        let options = self.get_options().await;
        if options.create_dirs {
            tokio::fs::create_dir_all(&options.save_path).await?;
        }
        let concurrency_limit = options.concurrency as usize;
        let base_path = PathBuf::from(&options.save_path);

        let tasks = resources.into_iter().map(async |resource| {
            match self.download_task(resource, &base_path).await {
                Ok(_) => {}
                Err(e) => {
                    self.reporter
                        .operation_result(OperationType::Download, 500, format!("Failed to download resource: {}", e))
                        .await
                        .ok();
                }
            }
        });

        let downloads = futures::stream
            ::iter(tasks)
            .buffer_unordered(concurrency_limit)
            .collect::<Vec<_>>();

        downloads.await;

        // tokio::fs::remove_file(PathBuf::from(&options.save_path).join("downloading.json")).await?;

        Ok(())
    }

    async fn download_task(
        &self,
        resource: DownloadResource,
        base_path: &PathBuf
    ) -> Result<()> {
        let save_interval = tokio::time::Duration::from_secs(1);
        let mut last_save = tokio::time::Instant::now();
        let mut current_len = 0;
        if base_path.exists() {
            // 这里要调
            let metadata = tokio::fs::metadata(&base_path).await?;
            current_len = metadata.len();
        }

        let resolved = self.resolver.resolve(&resource).await?;

        let mut request = self.client.get(resolved.url.as_str());
        if current_len > 0 {
            request = request.header("Range", format!("bytes={}-", current_len));
        }
        let response = request.send().await?;

        if
            !response.status().is_success() &&
            response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let meta = DownloadMeta::from_headers(response.headers());

        // println!("Meta {:?}", meta);

        let file_path = self.generate_path(&resource, &resolved, &meta).await?;

        // println!("Downloading {} to {:?}", resolved.url, file_path);

        let total_size = response.content_length().unwrap_or(0) + current_len;

        let task_id: u32;
        match resource {
            DownloadResource::Url(url) => {
                // 根据字符串生成随机3232
                task_id = generate_task_id(&url);
            }
            DownloadResource::Id(id) => {
                // 尝试解析为u32，否则根据字符串生成随机u32
                task_id = id.parse().unwrap_or_else(|_| generate_task_id(&id));
            }
            DownloadResource::Params(params) => {
                // 根据拼接后的字符串生成随机3232
                task_id = generate_task_id(&params.join(""));
            }
            DownloadResource::Resolved(resolved) => {
                // 根据url生成随机3232
                task_id = generate_task_id(&resolved.url);
            }
        }

        let task_url = resolved.url.clone();

        let task = DownloadTask::new(task_id, task_url, file_path.clone(), total_size);

        {
            self.tasks.write().await.push(task.clone());
        }

        self.reporter.start_task(task_id, total_size).await?;

        let mut file = tokio::fs::OpenOptions
            ::new()
            .create(true)
            .append(true)
            .open(&file_path).await?;

        let mut downloaded = current_len;
        let mut stream = response.bytes_stream();

        let start_time = tokio::time::Instant::now();

        // println!("Start downloading");

        while let Some(chunk) = stream.next().await {
            let global_state = self.state.read().await;

            match *global_state {
                DownloaderState::Idle => {
                    self.transition_state(DownloaderState::Running).await?;
                    continue;
                }
                DownloaderState::Suspended => {
                    task.pause().await?;
                    let mut state_rx = self.state_notifier.subscribe();

                    drop(global_state);
                    loop {
                        tokio::select! {
                            state_result = tokio::time::timeout(tokio::time::Duration::from_millis(1000), state_rx.recv()) => {
                                match state_result {
                                    Ok(Ok(new_state)) => {
                                        if new_state != DownloaderState::Suspended {
                                            println!("OK in global resume rx {:?}", new_state);
                                            break;
                                        }
                                    }
                                    Ok(Err(_)) => { /* 通道关闭 */ 
                                     }
                                    Err(_) => { /* 超时继续检查 */ 
                                    }
                                }
                            }


                            _ = async {
                                let current_state = self.state.read().await;
                                if *current_state != DownloaderState::Suspended {
                                    self.state_notifier.send(*current_state).ok();
                                }
                            } => {}
                        }

                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                        let state = self.state.read().await;
                        if *state != DownloaderState::Suspended {
                            break;
                        }
                    }

                    task.resume().await?; // 应该就没事了
                }
                DownloaderState::Stopped => {
                    task.cancel().await?;
                    self.reporter.finish_task(task_id, DownloadResult::Canceled).await?;
                    self.save_state().await?;
                    return Ok(());
                }
                DownloaderState::Running => {}
            }

            let task_state = task.state.read().await;

            match *task_state {
                TaskState::Pending => {
                    drop(task_state);
                    task.start().await?;
                }
                TaskState::Paused => {
                    let mut state_rx = self.state_notifier.subscribe();
                    drop(task_state);
                    loop {
                        tokio::select! {
                            state_result = tokio::time::timeout(tokio::time::Duration::from_millis(1000), state_rx.recv()) => {
                                match state_result {
                                    Ok(Ok(new_state)) => {
                                        if new_state != DownloaderState::Suspended {
                                            println!("OK in task resume rx {:?}", new_state);
                                            break;
                                        }
                                    }
                                    Ok(Err(_)) => { /* 通道关闭 */ }
                                    Err(_) => { /* 超时继续检查 */ }
                                }
                            }
                            
                            _ = async {
                                let current_state = self.state.read().await;
                                if *current_state != DownloaderState::Suspended {
                                    self.state_notifier.send(*current_state).ok();
                                }
                            } => {}
                        }

                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                        let state = self.state.read().await;
                        if *state != DownloaderState::Suspended {
                            break;
                        }
                    }
                }
                TaskState::Canceled => {
                    self.reporter.finish_task(task_id, DownloadResult::Canceled).await?;
                    drop(task_state);
                    self.save_state().await?;
                    return Ok(());
                }
                TaskState::Failed | TaskState::Completed => {
                    break;
                }
                TaskState::Downloading => {}
            }

            let chunk = chunk?;
            downloaded += chunk.len() as u64;

            let progress = self.calculate_progress(downloaded, total_size, start_time);

            {
                *task.progress.lock().await = progress.clone();
            }

            self.reporter.update_progress(task_id, &progress).await?;

            if last_save.elapsed() >= save_interval {
                self.save_state().await?;
                last_save = tokio::time::Instant::now();
            }

            file.write_all(&chunk).await?;
        }

        let metadata = tokio::fs::metadata(&file_path).await?;
        if metadata.len() == total_size {
            task.transition_state(TaskState::Completed).await?;
            self.reporter.finish_task(task_id, DownloadResult::Success {
                path: file_path.clone(),
                size: total_size,
                duration: start_time.elapsed(),
            }).await?;
        } else {
            task.transition_state(TaskState::Failed).await?;
            self.reporter.finish_task(task_id, DownloadResult::Failed {
                error: "Downloaded size does not match expected size".to_string(),
                retryable: true,
            }).await?;
        }

        self.save_state().await?;

        Ok(())
    }

    pub async fn pause_task(&self, task_id: u32) -> Result<()> {
        let tasks = self.tasks.write().await;
        if let Some(task) = tasks.iter().find(|t| t.id == task_id) {
            task.pause().await?;
            Ok(())
        } else {
            Err(format!("Task {} not found", task_id).into())
        }
    }

    pub async fn resume_task(&self, task_id: u32) -> Result<()> {
        let tasks = self.tasks.write().await;
        if let Some(task) = tasks.iter().find(|t| t.id == task_id) {
            task.resume().await?;
            Ok(())
        } else {
            Err(format!("Task {} not found", task_id).into())
        }
    }

    pub async fn cancel_task(&self, task_id: u32) -> Result<()> {
        let tasks = self.tasks.write().await;
        if let Some(task) = tasks.iter().find(|t| t.id == task_id) {
            task.cancel().await?;
            Ok(())
        } else {
            Err(format!("Task {} not found", task_id).into())
        }
    }

    pub async fn save_state(&self) -> Result<()> {
        let tasks = self.tasks.write().await;
        let mut task_states = Vec::new();
        for task in tasks.iter() {
            let progress = task.progress.lock().await;
            let task_state = TaskStateRecord {
                id: task.id,
                url: task.url.clone(),
                downloaded_bytes: progress.bytes_downloaded,
                total_bytes: progress.total_bytes,
                file_path: task.file_path.clone(),
                state: task.state.read().await.clone(),
            };
            task_states.push(task_state);
        }
        let state = PersistentState {
            tasks: task_states,
        };

        let contents = serde_json::to_string(&state)?;

        let options = self.get_options().await;

        let downloading_path = PathBuf::from(&options.save_path).join("downloading.json");

        tokio::fs::write(downloading_path, contents).await?;

        Ok(())
    }

    pub async fn load_state(&self, state: PersistentState) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        for task_state in state.tasks {
            let task = DownloadTask::new(
                task_state.id,
                task_state.url,
                task_state.file_path,
                task_state.total_bytes
            );

            let start_time = tokio::time::Instant::now();
            let progress = self.calculate_progress(
                task_state.downloaded_bytes,
                task_state.total_bytes,
                start_time
            );

            *task.progress.lock().await = progress;

            *task.state.write().await = task_state.state;

            tasks.push(task);
        }
        Ok(())
    }

    pub async fn load_state_from_file(&mut self, state_path: String) -> Result<()> {
        if let Ok(contents) = tokio::fs::read_to_string(state_path).await {
            let state: PersistentState = serde_json::from_str(&contents)?;

            let mut tasks = self.tasks.write().await;
            for task_state in state.tasks {
                let task = DownloadTask::new(
                    task_state.id,
                    task_state.url,
                    task_state.file_path,
                    task_state.total_bytes
                );

                let start_time = tokio::time::Instant::now();
                let progress = self.calculate_progress(
                    task_state.downloaded_bytes,
                    task_state.total_bytes,
                    start_time
                );

                *task.progress.lock().await = progress;

                *task.state.write().await = task_state.state;

                tasks.push(task);
            }
        }
        Ok(())
    }
    fn calculate_progress(
        &self,
        downloaded: u64,
        total: u64,
        start_time: tokio::time::Instant
    ) -> DownloadProgress {
        let elapsed = start_time.elapsed();
        let (rate, remaining_time, progress) = rate_remaining_progress(downloaded, total, elapsed);

        DownloadProgress {
            bytes_downloaded: downloaded,
            total_bytes: total,
            rate,
            remaining_time,
            progress_percentage: progress * 100.0,
        }
    }
}

// Convenience function for multi-download
pub async fn download_urls(
    resources: Vec<DownloadResource>,
    options: DownloadOptions,
    resolver: Box<dyn ResourceResolver>,
    reporter: Box<dyn CombinedReporter>
) -> Result<()> {
    let downloader = Downloader::new(options, resolver, reporter);
    downloader.download_multi(resources).await
}

// Quick multi-download with default settings
pub async fn quick_download_multi(
    resources: Vec<DownloadResource>,
    resolver: Box<dyn ResourceResolver>,
    reporter: Box<dyn CombinedReporter>
) -> Result<()> {
    let config = DownloadOptions::default().with_save_path("fetch".to_string());

    download_urls(resources, config, resolver, reporter).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;
    use crate::reporters::tui::TuiReporter;
    use crate::resolvers::url::UrlResolver;

    #[tokio::test]
    async fn test_download_single() {
        let options = DownloadOptions::default().with_save_path("fetch".to_string());

        let resources = vec![
            DownloadResource::Url("https://www.google.com".to_string()),
            DownloadResource::Url("https://www.bing.com".to_string()),
            DownloadResource::Url("https://www.baidu.com".to_string())
        ];

        let downloader = Downloader::new(
            options,
            Box::new(UrlResolver::new()),
            Box::new(TuiReporter::new())
        );
        downloader.download_task(resources[0].clone(), &PathBuf::from("fetch")).await.unwrap();
    }

    #[tokio::test]
    async fn test_download_multi() {
        let options = DownloadOptions::default().with_save_path("fetch".to_string());
        let resources = vec![
            DownloadResource::Url("https://www.google.com".to_string()),
            DownloadResource::Url("https://www.bing.com".to_string()),
            DownloadResource::Url("https://www.baidu.com".to_string())
        ];
        let downloader = Downloader::new(
            options,
            Box::new(UrlResolver::new()),
            Box::new(TuiReporter::new())
        );
        downloader.start(resources).await.unwrap();
    }
    #[tokio::test]
    async fn test_download_control() {
        let options = DownloadOptions::default()
            .with_save_path("fetch".to_string())
            .with_concurrency(2);
        let resources = vec![
            DownloadResource::Url("https://www.google.com".to_string()),
            DownloadResource::Url("https://www.bing.com".to_string()),
            DownloadResource::Url("https://www.baidu.com".to_string())
        ];
        let downloader = Arc::new(
            Mutex::new(
                Downloader::new(options, Box::new(UrlResolver::new()), Box::new(TuiReporter::new()))
            )
        );

        let downloader_clone = Arc::clone(&downloader);
        tokio::spawn(async move {
            downloader_clone.lock().await.start(resources).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        downloader.lock().await.pause().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        downloader.lock().await.resume().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        downloader.lock().await.stop().await.unwrap();
    }
}
