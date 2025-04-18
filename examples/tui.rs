#![cfg(feature = "tui")]

use vielpork::base::enums::DownloadResource;
use vielpork::base::structs::DownloadOptions;
use vielpork::downloader::Downloader;
use vielpork::error::Result;
use vielpork::reporters::tui::TuiReporter;
use vielpork::resolvers::url::UrlResolver;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    let options: DownloadOptions = DownloadOptions::default()
        .with_save_path("fetch".to_string())
        .with_concurrency(3);

    let downloader = Arc::new(Mutex::new(Downloader::new(
        options,
        Box::new(UrlResolver::new()),
        Box::new(TuiReporter::new()),
    )));

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

    let downloader_clone = Arc::clone(&downloader);
    tokio::spawn(async move {
        downloader_clone
            .lock()
            .await
            .start(resources)
            .await
            .unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    downloader.lock().await.pause().await?;
    println!("Paused");
    println!("Resuming in 2 seconds");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    println!("Resuming");
    println!("");
    println!("");
    downloader.lock().await.resume().await?;

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
