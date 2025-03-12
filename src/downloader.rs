use crate::base::algorithms::rate_remaining_progress;
use crate::base::algorithms::{
    auto_filename, custom_directory, custom_filename, generate_task_id, organize_by_domain,
    organize_by_type,
};
use crate::base::enums::{
    AuthMethod, DownloadResource, DownloadResult, DownloaderState, OperationType, TaskState,
};
use crate::base::structs::{DownloadMeta, DownloadOptions, DownloadProgress, ResolvedResource};
use crate::base::traits::{CombinedReporter, ResourceResolver};
use crate::error::Result;
use crate::task::{DownloadTask, PersistentState, TaskStateRecord};
use crate::template::{TemplateContext, TemplateRenderer};
use futures::stream::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    options: Arc<RwLock<DownloadOptions>>,
    pub state: Arc<RwLock<DownloaderState>>,
    pub tasks: Arc<RwLock<Vec<DownloadTask>>>,
    resolver: Arc<Box<dyn ResourceResolver>>,
    reporter: Arc<Box<dyn CombinedReporter>>,
    state_notifier: tokio::sync::broadcast::Sender<DownloaderState>,
    cancel_token: tokio_util::sync::CancellationToken,
}

impl Downloader {
    pub fn new(
        options: DownloadOptions,
        resolver: Box<dyn ResourceResolver>,
        reporter: Box<dyn CombinedReporter>,
    ) -> Self {
        let client = reqwest::ClientBuilder::new()
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
            cancel_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    pub async fn transition_state(&self, new_state: DownloaderState) -> Result<()> {
        let mut current = self.state.write().await;

        let valid = match (*current, new_state) {
            (DownloaderState::Idle, DownloaderState::Idle) => true,
            (DownloaderState::Idle, DownloaderState::Running) => true,
            (DownloaderState::Running, DownloaderState::Suspended) => true,
            (DownloaderState::Suspended, DownloaderState::Running) => true,
            (DownloaderState::Stopped, DownloaderState::Idle) => true,
            (_, DownloaderState::Stopped) => true,
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
    pub async fn get_downloading_tasks(&self) -> Vec<DownloadTask> {
        let tasks = self.tasks.read().await;
        futures::stream::iter(tasks.iter())
            .filter_map(|t| async move {
                let state = t.state.read().await;
                if *state == TaskState::Downloading {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .await
    }
    //
    pub async fn optimize_resources(
        &self,
        resources: Vec<DownloadResource>,
        state: PersistentState,
    ) -> Vec<DownloadResource> {
        let mut optimized = Vec::new();
        let resolver = self.resolver.clone();
        // 先解析所有的资源，然后对比resolved_resources的url是否在state中且已完成或取消
        let resolved_resources = futures::future::join_all(
            resources
                .into_iter()
                .map(async |resource| resolver.resolve(&resource).await),
        )
        .await;

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
        self.init().await?;

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
                        0,
                        500,
                        format!("Download failed: {}", e),
                    )
                    .await
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
        self.tasks.write().await.clear();
        self.transition_state(DownloaderState::Stopped).await
    }
    pub async fn init(&self) -> Result<()> {
        self.tasks.write().await.clear();
        self.transition_state(DownloaderState::Idle).await
    }

    async fn generate_path(
        &self,
        resource: &DownloadResource,
        resolved: &ResolvedResource,
        meta: &DownloadMeta,
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
            "custom" => {
                custom_filename(
                    resource,
                    resolved,
                    &TemplateRenderer::new(),
                    meta,
                    template,
                    max_length,
                )
                .await?
            }
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

        let tasks =
            resources
                .into_iter()
                .map(async |resource| match self.download_task(resource).await {
                    Ok(_) => {}
                    Err(e) => {
                        self.reporter
                            .operation_result(
                                OperationType::Download,
                                0,
                                500,
                                format!("Failed to download resource: {}", e),
                            )
                            .await
                            .ok();
                    }
                });

        let downloads = futures::stream::iter(tasks)
            .buffer_unordered(concurrency_limit)
            .collect::<Vec<()>>();

        downloads.await;

        tokio::fs::remove_file(PathBuf::from(&options.save_path).join("downloading.json")).await?;

        self.reporter
            .operation_result(
                OperationType::Download,
                0,
                200,
                "All Tasks Completed".to_string(),
            )
            .await?;

        Ok(())
    }

    async fn download_task(&self, resource: DownloadResource) -> Result<()> {
        let global_state = self.state.read().await;
        if *global_state == DownloaderState::Stopped {
            return Ok(());
        }
        drop(global_state);

        let save_interval = tokio::time::Duration::from_secs(1);
        let mut last_save = tokio::time::Instant::now();

        let task_id: u32;
        match resource.clone() {
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
                // task_id = generate_task_id(&params.join(""));
                // 尝试解析第一个值为u32，否则根据拼接后的字符串生成随机u32
                task_id = params[0]
                    .parse()
                    .unwrap_or_else(|_| generate_task_id(&params.join("")));
            }
            DownloadResource::HashMap(hashmap) => {
                // 根据拼接后值的字符串生成随机3232
                // task_id = generate_task_id(&hashmap.values().cloned().collect::<Vec<_>>().join(""));
                // 尝试解析第一个id键的值为u32，否则根据拼接后的字符串生成随机u32
                task_id = hashmap
                    .get("id")
                    .unwrap_or(&"".to_string())
                    .parse()
                    .unwrap_or_else(|_| {
                        generate_task_id(&hashmap.values().cloned().collect::<Vec<_>>().join(""))
                    });
            }
            DownloadResource::Resolved(resolved) => {
                // 直接使用resolved的id
                task_id = resolved.id;
            }
        }

        let resolved = self.resolver.resolve(&resource).await?;

        let mut pre_request = self.client.get(resolved.url.as_str());

        if resolved.headers.len() > 0 {
            for (key, value) in resolved.headers.iter() {
                pre_request = pre_request.header(key, value);
            }
        }

        if let Some(auth) = &resolved.auth {
            match auth {
                AuthMethod::Basic { username, password } => {
                    let value = format!("{}:{}", username, password);
                    let base64 = base64_simd::STANDARD;
                    let encoded = base64.encode_to_string(value.as_bytes());
                    let header = format!("Basic {}", encoded);
                    pre_request = pre_request.header("Authorization", header);
                }
                AuthMethod::Bearer { token } => {
                    pre_request = pre_request.header("Authorization", format!("Bearer {}", token));
                }
                AuthMethod::ApiKey { key, header } => {
                    pre_request = pre_request.header(header, key);
                }
                AuthMethod::None => {}
            }
        }

        let pre_response = pre_request.send().await?;
        if !pre_response.status().is_success() {
            return Err(format!("HTTP error: {}", pre_response.status()).into());
        }
        let meta = DownloadMeta::from_headers(pre_response.headers());
        drop(pre_response);

        let file_path = self.generate_path(&resource, &resolved, &meta).await?;

        let total_size = meta.expected_size.unwrap_or(0);
        let mut current_len = 0;
        if file_path.exists() {
            let metadata = tokio::fs::metadata(&file_path).await?;
            current_len = metadata.len();
        }

        if current_len == total_size {
            self.reporter.start_task(task_id, total_size).await?;
            self.reporter
                .finish_task(
                    task_id,
                    DownloadResult::Success {
                        path: file_path.clone(),
                        size: total_size,
                        duration: tokio::time::Duration::from_secs(0),
                    },
                )
                .await?;
            return Ok(());
        }

        let mut request = self.client.get(resolved.url.as_str());
        if current_len > 0 {
            request = request.header("Range", format!("bytes={}-", current_len));
        }

        if resolved.headers.len() > 0 {
            for (key, value) in resolved.headers.iter() {
                request = request.header(key, value);
            }
        }
        if let Some(auth) = &resolved.auth {
            match auth {
                AuthMethod::Basic { username, password } => {
                    let value = format!("{}:{}", username, password);
                    let base64 = base64_simd::STANDARD;
                    let encoded = base64.encode_to_string(value.as_bytes());
                    let header = format!("Basic {}", encoded);
                    request = request.header("Authorization", header);
                }
                AuthMethod::Bearer { token } => {
                    request = request.header("Authorization", format!("Bearer {}", token));
                }
                AuthMethod::ApiKey { key, header } => {
                    request = request.header(header, key);
                }
                AuthMethod::None => {}
            }
        }
        let response = request.send().await?;

        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let task_url = resolved.url.clone();

        let task = DownloadTask::new(task_id, task_url, file_path.clone(), total_size);

        {
            self.tasks.write().await.push(task.clone());
        }

        self.reporter.start_task(task_id, total_size).await?;

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await?;

        let mut downloaded = current_len;
        let mut stream = response.bytes_stream();

        let start_time = tokio::time::Instant::now();

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
                                            break;
                                        }
                                    }
                                    Ok(Err(_)) => { /* 通道关闭 */
                                        self.reporter.operation_result(OperationType::Download, task_id, 500, "State channel closed".to_string()).await.ok();
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

                    task.resume().await?;
                }
                DownloaderState::Stopped => {
                    task.cancel().await?;
                    self.reporter
                        .operation_result(
                            OperationType::Download,
                            task_id,
                            200,
                            "Download stopped".to_string(),
                        )
                        .await
                        .ok();
                    self.reporter
                        .finish_task(task_id, DownloadResult::Canceled)
                        .await?;
                    self.save_state().await?;
                    self.cancel_token.cancel();
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
                                            break;
                                        }
                                    }
                                    Ok(Err(_)) => { /* 通道关闭 */
                                        self.reporter.operation_result(OperationType::Download, task_id, 500, "State channel closed".to_string()).await.ok();
                                    }
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
                    self.reporter
                        .finish_task(task_id, DownloadResult::Canceled)
                        .await?;
                    self.reporter
                        .operation_result(
                            OperationType::Download,
                            task_id,
                            200,
                            "Download canceled".to_string(),
                        )
                        .await
                        .ok();
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

        file.sync_all().await?;
        drop(file);

        let final_size = tokio::fs::metadata(&file_path).await?;
        if final_size.len() == total_size {
            task.transition_state(TaskState::Completed).await?;
            self.reporter
                .operation_result(
                    OperationType::Download,
                    task_id,
                    200,
                    "Download task success".to_string(),
                )
                .await
                .ok();
            self.reporter
                .finish_task(
                    task_id,
                    DownloadResult::Success {
                        path: file_path.clone(),
                        size: total_size,
                        duration: start_time.elapsed(),
                    },
                )
                .await?;
        } else {
            tokio::fs::remove_file(&file_path).await?;
            task.transition_state(TaskState::Failed).await?;
            self.reporter
                .operation_result(
                    OperationType::Download,
                    task_id,
                    500,
                    "Downloaded size mismatch".to_string(),
                )
                .await
                .ok();
            self.reporter
                .finish_task(
                    task_id,
                    DownloadResult::Failed {
                        error: format!(
                            "Downloaded size mismatch: {} != {}",
                            final_size.len(),
                            total_size
                        ),
                        retryable: true,
                    },
                )
                .await?;
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
        let state = PersistentState { tasks: task_states };

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
                task_state.total_bytes,
            );

            let start_time = tokio::time::Instant::now();
            let progress = self.calculate_progress(
                task_state.downloaded_bytes,
                task_state.total_bytes,
                start_time,
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
                    task_state.total_bytes,
                );

                let start_time = tokio::time::Instant::now();
                let progress = self.calculate_progress(
                    task_state.downloaded_bytes,
                    task_state.total_bytes,
                    start_time,
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
        start_time: tokio::time::Instant,
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
    reporter: Box<dyn CombinedReporter>,
) -> Result<()> {
    let downloader = Downloader::new(options, resolver, reporter);
    downloader.download_multi(resources).await
}

// Quick multi-download with default settings
pub async fn quick_download_multi(
    resources: Vec<DownloadResource>,
    resolver: Box<dyn ResourceResolver>,
    reporter: Box<dyn CombinedReporter>,
) -> Result<()> {
    let config = DownloadOptions::default().with_save_path("fetch".to_string());

    download_urls(resources, config, resolver, reporter).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reporters::tui::TuiReporter;
    use crate::resolvers::url::UrlResolver;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_download_single() {
        let options = DownloadOptions::default().with_save_path("fetch".to_string());

        let resources = vec![
            DownloadResource::Url("https://www.google.com".to_string()),
            DownloadResource::Url("https://www.bing.com".to_string()),
            DownloadResource::Url("https://www.baidu.com".to_string()),
        ];

        let downloader = Downloader::new(
            options,
            Box::new(UrlResolver::new()),
            Box::new(TuiReporter::new()),
        );
        downloader
            .download_task(resources[0].clone())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_download_multi() {
        let options = DownloadOptions::default().with_save_path("fetch".to_string());
        let resources = vec![
            DownloadResource::Url("https://www.google.com".to_string()),
            DownloadResource::Url("https://www.bing.com".to_string()),
            DownloadResource::Url("https://www.baidu.com".to_string()),
        ];
        let downloader = Downloader::new(
            options,
            Box::new(UrlResolver::new()),
            Box::new(TuiReporter::new()),
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
            DownloadResource::Url("https://www.baidu.com".to_string()),
        ];
        let downloader = Arc::new(Mutex::new(Downloader::new(
            options,
            Box::new(UrlResolver::new()),
            Box::new(TuiReporter::new()),
        )));

        let downloader_clone = Arc::clone(&downloader);
        tokio::spawn(async move {
            downloader_clone
                .lock()
                .await
                .start(resources)
                .await
                .unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        downloader.lock().await.pause().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        downloader.lock().await.resume().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        downloader.lock().await.stop().await.unwrap();
    }
}
