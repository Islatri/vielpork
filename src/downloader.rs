use crate::task::{ DownloadTask, PersistentState, TaskStateRecord };
use crate::base::traits::CombinedReporter;
use crate::base::algorithms::rate_remaining_progress;
use crate::base::enums::{DownloaderState, TaskState,FinishType};
use crate::base::structs::{DownloadProgress, DownloadOptions };
use crate::error::Result;

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::io::AsyncWriteExt;
use futures::stream::StreamExt;
use std::path::PathBuf;

#[derive(Clone)]
pub struct BeatmapDownloader {
    client: reqwest::Client,
    options: Arc<RwLock<DownloadOptions>>,
    state: Arc<RwLock<DownloaderState>>, 
    tasks: Arc<RwLock<Vec<DownloadTask>>>, 
    reporter: Arc<Box<dyn CombinedReporter>>, 
    state_notifier: tokio::sync::broadcast::Sender<DownloaderState>,
}

impl BeatmapDownloader {
    pub fn new(options: DownloadOptions,reporter:Box<dyn CombinedReporter>) -> Self {
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
            Err(
                    format!("Cannot transition from {:?} to {:?}", *current, new_state)
                .into()
            )
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
    pub async fn start(&self, beatmapset_ids: Vec<u32>,urls: Vec<String>) -> Result<()> {

        let options = self.get_options().await;
        let save_path = &options.save_path;
        let state_path = PathBuf::from(save_path).join("downloading.json");
        let new_ids: Vec<u32>;

        if state_path.exists() {
            let contents = tokio::fs::read_to_string(&state_path).await?;
            let state: PersistentState = serde_json::from_str(&contents)?;
            self.load_state(state).await?;
            new_ids = futures::future
                ::join_all(
                    beatmapset_ids.into_iter().map(async |id: u32| {
                        let tasks = self.tasks.read().await;
                        let task = tasks.iter().find(|t| t.beatmapset_id == id);
                        match task {
                            Some(t) => {
                                let state = t.state.read().await;
                                if *state != TaskState::Completed || *state != TaskState::Canceled {
                                    Some(id)
                                } else {
                                    None
                                }
                            }
                            None => Some(id),
                        }
                    })
                ).await
                .into_iter()
                .filter_map(|id| id)
                .collect();
        } else {
            new_ids = beatmapset_ids;
        }

        self.transition_state(DownloaderState::Running).await?;

        let options = self.get_options().await;
        let downloader = self.clone();
        println!("Downloading {} beatmapsets", new_ids.len());

        let reporter = self.reporter.clone();
        tokio::spawn(async move {
            if let Err(e) = downloader.download_multi(new_ids,urls, options.concurrency as usize).await {
                reporter.operation_result(0, false, format!("Download failed: {}", e)).await.ok();
                eprintln!("Download failed: {}", e);
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
    pub async fn download_multi(
        &self,
        beatmapset_ids: Vec<u32>,
        urls: Vec<String>,
        concurrency_limit: usize
    ) -> Result<()> {
        let options = self.get_options().await;
        if options.create_dirs {
            tokio::fs::create_dir_all(&options.save_path).await?;
        }

        let default_url = String::new();
        let downloads = futures::stream
            ::iter(
                beatmapset_ids.into_iter().map(async |beatmapset_id| {
                    let file_path = PathBuf::from(&options.save_path).join(
                        format!("{}.osz", beatmapset_id)
                    );

                    let url = urls.get(beatmapset_id as usize).unwrap_or(&default_url);

                    match self.download_task(url,beatmapset_id,  &file_path).await {
                        Ok(_) => {
                        }
                        Err(e) => {
                            eprintln!("Failed to download beatmapset {}: {}", beatmapset_id, e);
                        }
                    }
                })
            )
            .buffer_unordered(concurrency_limit)
            .collect::<Vec<()>>();

        downloads.await;

        // tokio::fs::remove_file(PathBuf::from(&options.save_path).join("downloading.json")).await?;


        Ok(())
    }


    async fn download_task(
        &self,
        url: &str,
        beatmapset_id: u32,
        file_path: &PathBuf
    ) -> Result<()> {
        let save_interval = tokio::time::Duration::from_secs(1);
        let mut last_save = tokio::time::Instant::now();
        let mut current_len = 0;
        if file_path.exists() {
            let metadata = tokio::fs::metadata(&file_path).await?;
            current_len = metadata.len();
        }

        let mut request = self.client.get(url);
        if current_len > 0 {
            request = request.header("Range", format!("bytes={}-", current_len));
        }
        let response = request.send().await?;

        if
            !response.status().is_success() &&
            response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(
                format!("HTTP error: {}", response.status()).into()
            );
        }

        let total_size = response.content_length().unwrap_or(0) + current_len;

        let task = DownloadTask::new(beatmapset_id, file_path.clone(), total_size);

        {
            self.tasks.write().await.push(task.clone());
        }

        self.reporter.start_task(beatmapset_id, total_size).await?;

        let mut file = tokio::fs::OpenOptions
            ::new()
            .create(true)
            .append(true)
            .open(&file_path).await?;

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
                    self.reporter.finish_task(beatmapset_id,FinishType::Canceled).await?;
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
                    self.reporter.finish_task(beatmapset_id,FinishType::Canceled).await?;
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

            self.reporter.update_progress(beatmapset_id, &progress).await?;

            if last_save.elapsed() >= save_interval {
                self.save_state().await?;
                last_save = tokio::time::Instant::now();
            }

            file.write_all(&chunk).await?;
        } 



        let metadata = tokio::fs::metadata(&file_path).await?;
        if metadata.len() == total_size {
            task.transition_state(TaskState::Completed).await?;
            self.reporter.finish_task(beatmapset_id, FinishType::Success).await?;
        } else {
            task.transition_state(TaskState::Failed).await?;
            self.reporter.finish_task(beatmapset_id, FinishType::Failed).await?;
        }

        self.save_state().await?;

        Ok(())
    }

    pub async fn pause_task(&self, beatmapset_id: u32) -> Result<()> {
        let tasks = self.tasks.write().await;
        if let Some(task) = tasks.iter().find(|t| t.beatmapset_id == beatmapset_id) {
            task.pause().await?;
            Ok(())
        } else {
            Err(format!("Task {} not found", beatmapset_id).into())
        }
    }

    pub async fn resume_task(&self, beatmapset_id: u32) -> Result<()> {
        let tasks = self.tasks.write().await;
        if let Some(task) = tasks.iter().find(|t| t.beatmapset_id == beatmapset_id) {
            task.resume().await?;
            Ok(())
        } else {
            Err(format!("Task {} not found", beatmapset_id).into())
        }
    }

    pub async fn cancel_task(&self, beatmapset_id: u32) -> Result<()> {
        let tasks = self.tasks.write().await;
        if let Some(task) = tasks.iter().find(|t| t.beatmapset_id == beatmapset_id) {
            task.cancel().await?;
            Ok(())
        } else {
            Err(format!("Task {} not found", beatmapset_id).into())
        }
    }

    pub async fn save_state(&self) -> Result<()> {
        let tasks = self.tasks.write().await;
        let mut task_states = Vec::new();
        for task in tasks.iter() {
            let progress = task.progress.lock().await;
            let task_state = TaskStateRecord {
                beatmapset_id: task.beatmapset_id,
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
                task_state.beatmapset_id,
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
                    task_state.beatmapset_id,
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
pub async fn download_beatmaps(
    beatmapset_ids: Vec<u32>,
    concurrency_limit: usize,
    options: DownloadOptions,
    reporter:Box<dyn CombinedReporter>
) -> Result<()> {
    let downloader = BeatmapDownloader::new(options,reporter);
    downloader.download_multi(beatmapset_ids,vec![], concurrency_limit).await
}

// Quick multi-download with default settings
pub async fn quick_download_multi(
    beatmapset_ids: Vec<u32>,
    concurrency_limit: usize,
    reporter:Box<dyn CombinedReporter>
) -> Result<()> {

    let config = DownloadOptions::default()
        .with_save_path("fetch".to_string());

    download_beatmaps(beatmapset_ids, concurrency_limit, config,reporter).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;
    use crate::reporters::tui::TuiReporter;

    #[tokio::test]
    async fn test_download_single_no_osu_login() {
        let options = DownloadOptions::default()
            .with_save_path("fetch".to_string());

        let downloader = BeatmapDownloader::new(options,Box::new(TuiReporter::new()));
        downloader.download_task("",4587512,&PathBuf::new()).await.unwrap();
    }

    #[tokio::test]
    async fn test_download_multi() {
        let options = DownloadOptions::default()
            .with_save_path("fetch".to_string());

        let downloader = BeatmapDownloader::new(options,Box::new(TuiReporter::new()));
        downloader.start(vec![1234567, 114514, 1919810],vec![]).await.unwrap();
    }
    #[tokio::test]
    async fn test_download_control() {
        let options = DownloadOptions::default()
            .with_save_path("fetch".to_string())
            .with_concurrency(2);

        let downloader = Arc::new(Mutex::new(BeatmapDownloader::new(options,Box::new(TuiReporter::new()))));

        // 控制下载启停，断点续联
        let downloader_clone = Arc::clone(&downloader);
        tokio::spawn(async move {
            downloader_clone.lock().await.start(vec![1234567, 114514, 1919810],vec![]).await.unwrap();
        });

        // 但是他是直接顺序执行了，没有暂停下载，哦哦，可能是单点暂停还没写，但是之前设置了单线程
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        downloader.lock().await.pause().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        downloader.lock().await.resume().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        downloader.lock().await.stop().await.unwrap();
    }
}
