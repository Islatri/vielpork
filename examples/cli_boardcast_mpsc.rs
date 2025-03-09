use vielpork::downloader::Downloader;
use vielpork::reporters::cli_boardcast_mpsc::CliReporterBoardcastMpsc;
use vielpork::resolvers::url::UrlResolver;
use vielpork::base::structs::DownloadOptions;
use vielpork::base::enums::{DownloadResource, ProgressEvent};
use vielpork::error::Result;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
    async fn main() -> Result<()> {
        let options: DownloadOptions = DownloadOptions::default()
            .with_save_path("fetch".to_string())
            .with_concurrency(3);
        let reporter = Arc::new(CliReporterBoardcastMpsc::new(128));

        let downloader = Arc::new(Mutex::new(Downloader::new(options, Box::new(UrlResolver::new()), Box::new((*reporter).clone()))));


        let mut rx = reporter.subscribe_mpsc();
    
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                
                match event {
                    ProgressEvent::Start { task_id, total } => {
                        println!(
                            "Starting download of beatmapset {} with total size {}",
                            task_id,
                            total
                        );
                    }
                    ProgressEvent::Update { task_id, progress } => {
                        println!(
                            "Downloading beatmapset {} - {}%",
                            task_id,
                            progress.progress_percentage
                        );
                    }
                    
                    ProgressEvent::Finish { task_id,finish } => {
                        println!("Finished downloading beatmapset {}", task_id);
                        println!("Finish type: {:?}",finish);
                    }
                    ProgressEvent::OperationResult { operation, code, message } => {
                        println!("Operation result: {:?} - {} - {}", operation, code, message);
                    }
                }
            }
        });

        let resources = vec![
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
        ];
        // 控制下载启停，断点续联
        let downloader_clone = Arc::clone(&downloader);
        tokio::spawn(async move {
            downloader_clone.lock().await.start(resources).await.unwrap();
            
        });

        
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        downloader.lock().await.pause().await?;
        println!("Paused");
        println!("Resuming in 2 seconds");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        println!("Resuming");
        downloader.lock().await.resume().await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        downloader.lock().await.pause().await?;
        println!("Paused");
        println!("Resuming in 2 seconds");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        println!("Resuming");
        downloader.lock().await.resume().await?;


        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        downloader.lock().await.stop().await?;
        
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        Ok(())
    }