<!-- markdownlint-disable MD033 MD041 MD045 -->
<p align="center" dir="auto">
    <img style="height:240px;width:280px"  src="https://s2.loli.net/2025/03/09/ho9EQVWa8zYxP2J.jpg" alt="Logo逃走啦~"/>
</p>

<p align="center">
  <h1 align="center">Vielpork 🚀</h1>
  <p align="center">Rust编写的高性能多线程HTTP下载器库，具有可自定义的报告器和资源解析策略。</p>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/" target="_blank"><img src="https://img.shields.io/badge/Rust-1.85%2B-blue"/></a>
  <a href="https://crates.io/crates/vielpork" target="_blank"><img src="https://img.shields.io/crates/v/vielpork"/></a>
  <a href="https://docs.rs/vielpork" target="_blank"><img src="https://img.shields.io/docsrs/vielpork/0.1.2"/></a>
  <a href="https://github.com/islatri/vielpork" target="_blank"><img src="https://img.shields.io/badge/License-MIT-green.svg"/></a>

</p>

<p align="center">
  <hr />

[中文版本](README.md) | [English Version](README_EN.md)

Vielpork是一个高性能的多线程HTTP下载器，由Rust编写,具有可自定义的报告器和资源解析策略。它提供：

- 🚀 多线程下载以获得最大速度
- 📊 多种内置报告器适配大部分场景
- 📦 丰富的路径策略选项与模板命名支持
- 🔧 为不同下载场景提供可定制的资源解析策略
- ⏯️ 支持全局与单个任务的暂停/恢复功能

# 文档

1. English: [https://hakochest.github.io/vielpork-en/](https://hakochest.github.io/vielpork-en/)
2. 中文: [https://hakochest.github.io/vielpork-cn/](https://hakochest.github.io/vielpork-cn/)

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

## 相关项目

- [osynic_downloader](https://github.com/osynicite/osynic_downloader): 基于vielpork的osu!谱面下载器，包含工具库和TUI应用

![osynic_downloader.gif](https://s2.loli.net/2025/03/10/hasqOmgctyG4TWd.gif)

## 核心特性

- **多线程架构**：利用Rust的异步运行时进行并发的分块下载
- **可扩展的报告系统**：
  - 内置报告器：TUI进度条，CLI Boardcast 转 Mpsc 通道
  - 通过Reporter trait实现自定义报告器
- **智能解析**：
  - 通过Resolver trait进行自定义解析逻辑
- **恢复与韧性**：
  - 继续上次中断的下载
- **进度跟踪**：
  - 实时速度计算
  - ETA估算
  - 详细的传输统计

## 安装

添加到您的`Cargo.toml`：

```toml
[dependencies]
vielpork = "0.1.2"
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

## 内置选项

### 报告器

- **TuiReporter**：基于`indicatif`库的终端进度条
- **CliReporterBoardcastMpsc**：一个广播进度更新到多个通道并用单个通道完成的报告器（使用示例：在Tonic gRPC服务器流中，rx类型只能是mpsc，因此我们需要将进度广播到mpsc通道，然后通过服务器将其发送到客户端）

### 解析器

- **UrlResolver**：一个从URL下载资源的解析器，只是reqwest的简单包装

## 自定义组件

您可以在`vielpork::base::traits`中查看所有trait并实现自己的组件。

### 自定义报告器

- 这里有2个需要使用async_trait实现的trait：
  - `ProgressReporter`：允许报告器处理进度更新的trait
  - `ResultReporter`：允许报告器处理操作或任务的结果的trait

### 自定义解析器

- 这里只有1个需要使用async_trait实现的trait：
  - `ResourceResolver`：允许解析器从特定来源下载资源的trait

## 🤝 贡献指南

这个库是差不多一个上午写完的，所以肯定还有很多地方需要改进，目前也只是满足了我自己的项目需求，不能保证完全符合所有人的需求。

以及我是一名Rust初学者，代码其中可能有很多不规范的地方，因为学习和编程的时间有限，给大家带来的困扰，还请见谅( >﹏<。)。

所以，如果代码有任何问题，或者你有任何建议，欢迎提交PR或者Issue，我会尽快处理~

如果你想贡献代码，请遵循以下规则：

- 遵循Rust官方编码规范
- 新增功能需附带测试用例
- 提交前运行`cargo fmt`和`cargo clippy`

## 📜 开源协议

本项目基于 [MIT License](LICENSE) 开源，请尊重原作者的著作权。

## 后记（或者说最开始的序章）

最开始找到了viel这个词，后面想了下rufen、ekstase、reichen

但是正在我还在犹豫不决的时候，好朋友来寝室送了我一纸杯的熏猪肉丝

所以我就直接取名叫做vielpork了，这个名字的意思是很多猪肉丝

但如果是功能描述的话，这个下载器主打的是多报道通道下载，所以也是很多报道

report的vielpork很接近，也还不错

对于连续吃了一个星期免费粥的我来说，这个名字已经很好了

哦对了，水煮肉片也可以算是VielPork了
