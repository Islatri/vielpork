<p align="center" dir="auto">
    <img style="height:240px;width:280px"  src="https://s2.loli.net/2025/03/09/ho9EQVWa8zYxP2J.jpg" alt="Logo逃走啦~"/>
</p>

<p align="center">
  <h1 align="center">Vielpork 🚀</h1>
  <p align="center">A high-performance multi-threaded HTTP downloader with extensible reporting and resolution strategies.</p>
</p>

<p align="center">
  <a href="https://crates.io/crates/vielpork" target="_blank"><img src="https://img.shields.io/crates/v/vielpork"/></a>
  <a href="https://docs.rs/vielpork" target="_blank"><img src="https://img.shields.io/docsrs/vielpork/0.1.0"/></a>
  <a href="https://github.com/islatri/vielpork" target="_blank"><img src="https://img.shields.io/badge/License-MIT-green.svg"/></a>

</p>

<p align="center">
  <hr />

**Vielpork** is a Rust-powered HTTP downloader designed for performance and extensibility. It offers:

- 🚀 Multi-threaded downloading for maximum speed
- 📊 Flexible reporting system with multiple built-in options
- 🔧 Customizable resolution strategies for different network scenarios
- ⏯️ Pause/resume functionality with checkpoint support


```mermaid
stateDiagram-v2
    [*] --> GlobalInit
    GlobalInit --> GlobalRunning: start_all()
    GlobalRunning --> GlobalSuspended: pause_all()
    GlobalSuspended --> GlobalRunning: resume_all()
    GlobalRunning --> GlobalStopped: cancel_all()
    GlobalStopped --> [*]
    
    state TaskStates {
        [*] --> TaskPending
        TaskPending --> TaskDownloading: start_task()
        TaskDownloading --> TaskPaused: pause_task()
        TaskPaused --> TaskDownloading: resume_task()
        TaskDownloading --> TaskCanceled: cancel_task()
        TaskDownloading --> TaskCompleted: finish()
        TaskPaused --> TaskCanceled: cancel_task()
        TaskCanceled --> [*]
        TaskCompleted --> [*]
    }
    
    GlobalSuspended --> TaskPaused : propagate
    GlobalStopped --> TaskCanceled : propagate
```

# Documentation

1. English: [https://hakochest.github.io/vielpork-en/](https://hakochest.github.io/vielpork-en/)
2. 中文: [https://hakochest.github.io/vielpork-cn/](https://hakochest.github.io/vielpork-cn/)

## Features

### Core Capabilities

- **Multi-threaded Architecture**: Leverage Rust's async runtime for concurrent chunk downloads
- **Extensible Reporting**:
  - Built-in reporters: TUI progress bar, CLI broadcast mpsc channel
  - Custom reporter implementation via trait
- **Smart Resolution**:
  - Custom resolution logic through Resolver trait
- **Recovery & Resilience**:
  - Resume interrupted downloads
- **Progress Tracking**:
  - Real-time speed calculations
  - ETA estimation
  - Detailed transfer statistics

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
vielpork = "0.1.0"
```

## Quick Start

```rust
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

    let downloader = Downloader::new(options, Box::new(UrlResolver::new()), Box::new(TuiReporter::new()));

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

    downloader.start(resources).await?;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        // Because of the async nature of the downloader, we need to keep the main thread alive
    }

    Ok(())
}
```

## Built-in Options

### Reporters

- **TuiReporter**: A terminal-based progress bar based on the `indicatif` library
- **CliReporterBoardcastMpsc**: A reporter that broadcasts progress updates to multiple channels and finalizes them with a single channel ( Usage Example: In Tonic gRPC server streaming, the rx type can only be mpsc, so we need to broadcast the progress to a mpsc channel, then send it to the client through the server)

### Resolvers

- **UrlResolver**: A resolver that downloads resources from a URL, just a simple wrapper around reqwest

## Custom Components

You can see all traits at `vielpork::base::traits` and implement your own components.

### Custom Reporter

- Here are 2 traits that you need to implement with async_trait:
  - `ProgressReporter`: A trait that allows the reporter to handle progress updates
  - `ResultReporter`: A trait that allows the reporter to handle the results of operations or tasks

### Custom Resolver

- Here is only 1 trait that you need to implement with async_trait:
  - `ResourceResolver`: A trait that allows the resolver to download resources from a specific source

# 后记（或者说最开始的序章）

最开始找到了viel这个词，后面想了下rufen、ekstase、reichen

但是正在我还在犹豫不决的时候，好朋友来寝室送了我一纸杯的熏猪肉丝

所以我就直接取名叫做vielpork了，这个名字的意思是很多猪肉丝

但如果是功能描述的话，这个下载器主打的是多报道通道下载，所以也是很多报道

report的vielpork很接近，也还不错

对于连续吃了一个星期免费粥的我来说，这个名字已经很好了
