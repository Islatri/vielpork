use std::time::Duration;

// 函数一：计算当前下载速度
pub fn rate(
    downloaded: u64,
    elapsed: Duration,
) -> f64 {
    let elapsed = elapsed.as_secs_f64();
    let downloaded = downloaded as f64;
    let rate = downloaded / elapsed;
    rate
}

// 函数二：计算剩余时间
pub fn remaining_time(
    downloaded: u64,
    total: u64,
    elapsed: Duration,
) -> Duration {
    // 确保 downloaded 不大于 total
    if downloaded >= total {
        return Duration::from_secs(0);
    }
    
    let rate = rate(downloaded, elapsed);
    // 确保速率大于 0
    if rate <= 0.0 {
        return Duration::from_secs(0);
    }
    
    let remaining = total as f64 - downloaded as f64;
    let remaining_time = remaining / rate;
    Duration::from_secs_f64(remaining_time)
}

// 函数三：计算下载进度
pub fn progress(downloaded: u64, total: u64) -> f64 {
    let downloaded = downloaded as f64;
    let total = total as f64;
    let progress = downloaded / total;
    progress
}

// 函数四：计算下载速度、剩余时间、下载进度
pub fn rate_remaining_progress(
    downloaded: u64,
    total: u64,
    elapsed: Duration,
) -> (f64, Duration, f64) {
    let rate = rate(downloaded, elapsed);
    let remaining_time = remaining_time(downloaded, total, elapsed);
    let progress = progress(downloaded, total);
    (rate, remaining_time, progress)
}