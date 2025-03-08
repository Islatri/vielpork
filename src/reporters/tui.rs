
use crate::error::Result;
use crate::base::structs::DownloadProgress;
use crate::base::enums::FinishType;
use crate::base::traits::{ProgressReporter, ResultReporter};
use async_trait::async_trait;


#[cfg(feature = "tui")]
use indicatif::{ProgressBar, ProgressStyle,ProgressDrawTarget,MultiProgress};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const MAX_CONCURRENT_BARS: usize = 4;

// TUI 实现
#[cfg(feature = "tui")]
#[derive(Debug)]
pub struct TuiReporter {
    mp: MultiProgress,
    bars: Arc<Mutex<HashMap<u32, ProgressBar>>>,
}

impl TuiReporter {
    pub fn new() -> Self {
        let mp = Self::setup_global_progress();
        Self {
            mp,
            bars: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    fn setup_global_progress() -> MultiProgress {
        let mp = MultiProgress::new();
        mp.set_draw_target(ProgressDrawTarget::stdout());
        mp
    }

    // 私有方法用于获取或创建进度条
    async fn get_or_create_bar(&self, beatmapset_id: u32, total: u64) -> ProgressBar {
        let mut bars = self.bars.lock().await;
        // 但是有时候就只有两个进度条
        
        if bars.len() >= MAX_CONCURRENT_BARS {
            bars.retain(|_, bar| !bar.is_finished());
        }

        bars.entry(beatmapset_id)
            .or_insert_with(|| {
                let bar = ProgressBar::new(total);
                // 关键修改：将进度条添加到 MultiProgress 系统
                let bar = self.mp.add(bar); // 这行是核心修改
                bar.set_style(ProgressStyle::with_template(&format!(
                    "{{spinner:.green}} [{{bar:.cyan/blue}}] {{bytes}}/{{total_bytes}} ({}) {{msg}}",
                    beatmapset_id
                ))
                .unwrap() //迫不得已
                .progress_chars("#>-"));
                bar
            });
        
        bars.get(&beatmapset_id).unwrap().clone()
    }
}


#[async_trait]
impl ProgressReporter for TuiReporter {
    async fn start_task(&self, beatmapset_id: u32, total: u64) -> Result<()> {
        let bar = self.get_or_create_bar(beatmapset_id, total).await;
        bar.set_message("Downloading...");
        Ok(())
    }

    async fn update_progress(&self, beatmapset_id: u32, progress: &DownloadProgress) -> Result<()> {
        let bar = self.get_or_create_bar(beatmapset_id, progress.total_bytes).await;
        bar.set_position(progress.bytes_downloaded);

        let speed = if progress.rate > 1_000_000.0 {
            format!("{:.2} MB/s", progress.rate / 1_000_000.0)
        } else if progress.rate > 1_000.0 {
            format!("{:.2} KB/s", progress.rate / 1_000.0)
        } else {
            format!("{:.0} B/s", progress.rate)
        };

        // 格式化剩余时间
        let eta = if progress.remaining_time.as_secs() > 60 {
            format!("{}m {}s", 
                progress.remaining_time.as_secs() / 60,
                progress.remaining_time.as_secs() % 60)
        } else {
            format!("{}s", progress.remaining_time.as_secs())
        };
        
        bar.set_message(format!("Speed: {} | ETA: {}", speed,eta));

        
        Ok(())
    }

    async fn finish_task(&self, beatmapset_id: u32,finish: FinishType) -> Result<()> {
        let mut bars = self.bars.lock().await;
        if let Some(bar) = bars.remove(&beatmapset_id) {
            match finish {
                FinishType::Success => {
                    // 条的颜色变成绿色，还是#>-
                    bar.set_style(ProgressStyle::with_template(&format!(
                        "{{spinner:.green}} [{{bar:.green/blue}}] {{bytes}}/{{total_bytes}} ({}): {{msg}}",
                        beatmapset_id
                    ))?.progress_chars("#>-"));
                    bar.finish_with_message("✅ Done")
                },
                FinishType::Failed => {
                    // 条变成红色
                    bar.set_style(ProgressStyle::default_bar().template(&format!(
                        "{{spinner:.red}} [{{bar:.red/blue}}] {{bytes}}/{{total_bytes}} ({}): {{msg}}",
                        beatmapset_id
                    ))?.progress_chars("#>-"));
                    bar.set_message("❌ Failed")
                },
                FinishType::Canceled => {
                    // 条变成黄色
                    bar.set_style(ProgressStyle::default_bar().template(&format!(
                        "{{spinner:.red}} [{{bar:.yellow/blue}}] {{bytes}}/{{total_bytes}} ({}): {{msg}}",
                        beatmapset_id
                    ))?.progress_chars("#>-"));
                    bar.abandon_with_message("⛔ Canceled")
                },
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ResultReporter for TuiReporter {
    async fn operation_result(&self, correlation_id: u32, success: bool, message: String) -> Result<()> {
        let bars = self.bars.lock().await;
        if let Some(bar) = bars.get(&correlation_id) {
            if success {
                bar.set_message(message.to_string());
            } else {
                bar.set_message(message.to_string());
            }
        }
        Ok(())
    }
}