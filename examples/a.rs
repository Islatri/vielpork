use vielpork::downloader::Downloader;
use vielpork::reporters::tui::TuiReporter;
use vielpork::resolvers::url::UrlResolver;
use vielpork::base::structs::DownloadOptions;
use vielpork::base::enums::DownloadResource;
use vielpork::error::Result;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
    async fn main() -> Result<()> {
        let options: DownloadOptions = DownloadOptions::default()
            .with_save_path("fetch".to_string())
            .with_concurrency(3);

        let downloader = Arc::new(Mutex::new(Downloader::new(options, Box::new(UrlResolver::new()), Box::new(TuiReporter::new()))));

        let resources = vec![
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
            DownloadResource::Url("https://example.com".to_string()),
        ];


        // 控制下载启停，断点续联
        let downloader_clone = Arc::clone(&downloader);
        tokio::spawn(async move {
            downloader_clone.lock().await.start(resources).await.unwrap();
            
        });

        // downloader.lock().await.start(vec![114514,1919810,1001653,1538879]).await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        downloader.lock().await.pause().await?;
        println!("Paused");
        println!("Resuming in 2 seconds");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        println!("Resuming");
        println!("");
        println!("");
        downloader.lock().await.resume().await?;

        println!("Pausing task 1538879 in 2 seconds");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        downloader.lock().await.pause_task(1538879).await?;

        println!("Paused task 1538879");
        println!("Resuming task 1538879 in 2 seconds");
        println!("");
        println!("");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        downloader.lock().await.resume_task(1538879).await?;
        // println!("Resumed task 1538879");

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        println!("Canceling task 1538879 in 2 seconds");
        println!("");
        println!("");
        downloader.lock().await.cancel_task(1538879).await?;
        // println!("Canceled task 1538879");

        // 单个暂停然后遇到死锁了，但是好像又好了？，挺奇怪的
        // 单个取消好像没能提前设置好状态
        
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        downloader.lock().await.pause().await?;
        println!("Paused");
        println!("Resuming in 2 seconds");
        println!("");
        println!("");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        println!("Resuming");
        println!("");
        println!("");
        downloader.lock().await.resume().await?;


        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        println!("");
        println!("");
        
        println!("");
        println!("");
        println!("");
        println!("");
        println!("");
        println!("");
        println!("");
        
        println!("");
        println!("");
        downloader.lock().await.stop().await?;
        
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        Ok(())
    }