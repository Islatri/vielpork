<p align="center" dir="auto">
    <img style="height:240px;width:280px"  src="https://s2.loli.net/2025/03/09/ho9EQVWa8zYxP2J.jpg" alt="Logo逃走啦~"/>
</p>

<h1 align="center" tabindex="-1" class="heading-element" dir="auto">Vielpork</h1>

<p align="center">
  <a href="https://crates.io/crates/vielpork" target="_blank"><img src="https://img.shields.io/crates/v/vielpork"/></a>
  <a href="https://docs.rs/vielpork" target="_blank"><img src="https://img.shields.io/docsrs/vielpork/0.1.0"/></a>
  <a href="https://github.com/islatri/vielpork" target="_blank"><img src="https://img.shields.io/badge/License-MIT-green.svg"/></a>

</p>

<p align="center">
    A multi-threaded downloader with multi-reporter built in Rust
</p>

<hr />

最开始找到了viel这个词，后面想了下rufen、ekstase、reichen

但是正在我还在犹豫不决的时候，好朋友来寝室送了我一纸杯的熏猪肉丝

所以我就直接取名叫做vielpork了，这个名字的意思是很多猪肉丝

但如果是功能描述的话，这个下载器主打的是多报道通道下载，所以也是很多报道

report的vielpork很接近，也还不错

对于连续吃了一个星期免费粥的我来说，这个名字已经很好了

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
    
    GlobalPaused --> TaskPaused : propagate
    GlobalCanceling --> TaskCanceled : propagate
```
