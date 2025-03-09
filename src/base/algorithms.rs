use crate::error::Result;
use crate::base::structs::{ DownloadMeta, ResolvedResource, DownloadResource };
use crate::template::{ TemplateRenderer, TemplateContext };
// use crate::hash::{HashSource, HashFormat};

use std::time::Duration;
use std::collections::HashMap;
use chrono::Utc;
use std::path::{ Path, PathBuf, Component };

// 函数一：计算当前下载速度
pub fn rate(downloaded: u64, elapsed: Duration) -> f64 {
    let elapsed = elapsed.as_secs_f64();
    let downloaded = downloaded as f64;
    let rate = downloaded / elapsed;
    rate
}

// 函数二：计算剩余时间
pub fn remaining_time(downloaded: u64, total: u64, elapsed: Duration) -> Duration {
    // 确保 downloaded 不大于 total
    if downloaded >= total {
        return Duration::from_secs(0);
    }

    let rate = rate(downloaded, elapsed);
    // 确保速率大于 0
    if rate <= 0.0 {
        return Duration::from_secs(0);
    }

    let remaining = (total as f64) - (downloaded as f64);
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
    elapsed: Duration
) -> (f64, Duration, f64) {
    let rate = rate(downloaded, elapsed);
    let remaining_time = remaining_time(downloaded, total, elapsed);
    let progress = progress(downloaded, total);
    (rate, remaining_time, progress)
}

/// 解析Content-Disposition头获取文件名（支持RFC 5987编码）
pub fn parse_content_disposition(header_value: &str) -> Option<String> {
    let re = regex::Regex
        ::new(
            r#"(?x)
        (?:filename\*=(UTF-8|ISO-8859-1)'[^']*'(?P<enc_filename>[^;]+)) |
        (?:filename="(?P<quoted_filename>(?:\\"|[^"])+)") |
        (?:filename=(?P<plain_filename>[^;]+))
        "#
        )
        .unwrap();

    let captures = re.captures(header_value)?;

    // 优先处理RFC 5987编码的文件名
    if let (Some(encoding), Some(enc_filename)) = (captures.get(1), captures.name("enc_filename")) {
        return decode_encoded_filename(enc_filename.as_str(), encoding.as_str());
    }

    // 处理带引号的文件名
    if let Some(quoted) = captures.name("quoted_filename") {
        return Some(quoted.as_str().replace(r#"\""#, "\""));
    }

    // 处理普通文件名
    captures.name("plain_filename").map(|m| m.as_str().trim().to_string())
}

/// 解码RFC 5987编码的文件名
fn decode_encoded_filename(filename: &str, encoding: &str) -> Option<String> {
    let decoded = percent_encoding::percent_decode_str(filename).decode_utf8().ok()?;

    match encoding.to_uppercase().as_str() {
        "UTF-8" => Some(decoded.to_string()),
        "ISO-8859-1" => {
            let bytes: Vec<u8> = decoded.as_bytes().to_vec();
            Some(encoding_rs::ISO_8859_10.decode(&bytes).0.to_string())
        }
        _ => None,
    }
}

pub async fn organize_by_type(meta: &DownloadMeta) -> Result<PathBuf> {
    let mime_type = meta.content_type.as_deref().unwrap_or("application/octet-stream");
    let categories = match mime_type.split('/').next().unwrap() {
        "image" => "media/images",
        "video" => "media/videos",
        "audio" => "media/audio",
        "text" => "documents",
        "application" => "binaries",
        _ => "others",
    };
    Ok(PathBuf::from(categories))
}

pub async fn organize_by_domain(resolved: &ResolvedResource) -> Result<PathBuf> {
    let domain = reqwest::Url
        ::parse(&resolved.url)
        .map_err(|_| "Invalid URL")?
        .host_str()
        .map(|h| h.to_string())
        .unwrap_or_else(|| "unknown".into());
    Ok(PathBuf::from(domain))
}

pub async fn auto_filename(resolved: &ResolvedResource, meta: &DownloadMeta) -> Result<String> {
    // 优先从 Content-Disposition 头获取
    if
        let Some(disposition) = resolved.headers
            .iter()
            .find(|(k, _)| k == "Content-Disposition")
            .map(|(_, v)| v)
    {
        if let Some(filename) = parse_content_disposition(disposition) {
            return Ok(filename);
        }
    }

    // 从 URL 路径获取文件名
    if
        let Some(url_filename) = reqwest::Url
            ::parse(&resolved.url)
            .ok()
            .and_then(|u| u.path_segments().and_then(|s| s.last().map(|s| s.to_string())))
    {
        return Ok(url_filename.to_string());
    }

    // 生成随机文件名
    generate_random_filename(meta)
}

fn generate_random_filename(meta: &DownloadMeta) -> Result<String> {
    let ext = meta.suggested_filename
        .as_ref()
        .and_then(|s|
            PathBuf::from(s)
                .extension()
                .map(|e| e.to_os_string())
        )
        .and_then(|e| e.to_str().map(|s| s.to_string()))
        .unwrap_or("bin".to_string());
    let random_name = uuid::Uuid::new_v4().to_string();
    Ok(format!("{}.{}", random_name, ext))
}

fn sanitize_filename(name: &str) -> String {
    let replace_char = '_';
    let blacklist = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

    name.chars()
        .map(|c| if blacklist.contains(&c) { replace_char } else { c })
        .collect()
}

fn truncate_filename(name: &str, max_length: usize) -> String {
    let mut s = name.to_string();
    if s.len() > max_length {
        let ext_pos = s.rfind('.').unwrap_or(s.len());
        let (stem, ext) = s.split_at(ext_pos);

        let keep_len = max_length - ext.len() - 1; // 留出...的空间
        if keep_len > 0 {
            s = format!("{}...{}", &stem[..keep_len.min(stem.len())], ext);
        }
    }
    s
}

pub async fn custom_filename(
    resource: &DownloadResource,
    resolved: &ResolvedResource,
    renderer: &TemplateRenderer,
    meta: &DownloadMeta,
    template: &str,
    max_length: usize
) -> Result<String> {
    // 创建模板渲染上下文
    let parsed_url = reqwest::Url::parse(&resolved.url).ok();

    let domain = parsed_url.as_ref().and_then(|u| u.host_str().map(|s| s.to_string()));
    let context = TemplateContext {
        url: &resolved.url,
        domain: domain.as_deref(),
        filename: meta.suggested_filename.as_deref().unwrap_or_else(|| "file"),
        extension: meta.suggested_filename
            .as_ref()
            .and_then(|f| Path::new(f).extension())
            .and_then(|e| e.to_str()),
        meta,
        download_time: Utc::now(),
        custom_data: Some({
            let mut map = HashMap::new();
            // 添加资源特定数据
            if let DownloadResource::Id(id) = resource {
                map.insert("resource_id".into(), id.clone());
            }
            map
        }),
    };

    // 渲染模板
    let raw_name = renderer.render_path_template(template, &context).map_err(|e| e)?;

    // 清理和截断文件名
    let sanitized = sanitize_filename(&raw_name);
    let final_name = truncate_filename(&sanitized, max_length);

    Ok(final_name)
}

// Deprecated

// async fn hash_filename(
//     resource: &DownloadResource,
//     meta: &DownloadMeta,
// ) -> Result<String> {
//     // 获取哈希配置参数
//     let hash_config = self.options
//         .path_policy
//         .hash_config
//         .as_ref()
//         .ok_or("Hash config is not set")?;

//     // 根据策略生成哈希源
//     let hash_input = match hash_config.source {
//         HashSource::ResourceIdentifier => {
//             let mut hasher = hash_config.algorithm.create_hasher();
//             match resource {
//                 DownloadResource::Url(url) => hasher.update(url.as_bytes()),
//                 DownloadResource::Id(id) => hasher.update(id.as_bytes()),
//             }
//             hasher.finalize()
//         }
//         HashSource::ContentPreview => {
//             // 预下载前1MB计算哈希（需实现range请求）
//             let preview = self.fetch_content_preview(resource, 1024 * 1024).await?;
//             let mut hasher = hash_config.algorithm.create_hasher();
//             hasher.update(&preview);
//             hasher.finalize()
//         }
//         HashSource::FullContent => {
//             // 注意：此模式需要先下载完整内容
//             return Err("FullContent hash source is not implemented yet".into());
//         }
//     };

//     // 获取文件扩展名
//     let ext = meta.suggested_filename
//         .as_ref()
//         .and_then(|f| Path::new(f).extension())
//         .and_then(|e| e.to_str())
//         .unwrap_or_default();

//     // 格式化输出
//     let filename = match hash_config.format {
//         HashFormat::Hex => format!("{}_{}", hash_input, ext),
//         HashFormat::Base64 => {
//             let base64 = base64_simd::STANDARD;
//             let b64 = base64.encode_to_string(hash_input);
//             format!("{}_{}", b64, ext)
//         }
//         HashFormat::Custom(ref fmt) => fmt
//             .replace("{hash}", &hex::encode(hash_input))
//             .replace("{ext}", ext),
//     };

//     Ok(filename)
// }

/// 生成自定义目录结构
pub async fn custom_directory(
    template: &str,
    context: &TemplateContext<'_>,
    renderer: &TemplateRenderer
) -> Result<PathBuf> {
    let dir_path = renderer.render_path_template(template, context).map_err(|e| e)?;

    // 清理路径中的非法字符
    let sanitized_path = sanitize_path(&dir_path)?;

    // 分解路径组件并验证
    let components: Vec<&str> = sanitized_path.split('/').collect();
    if components.len() > 10 {
        return Err("Path too deep".into());
    }

    Ok(PathBuf::from(sanitized_path))
}

/// 路径清理和验证
fn sanitize_path(path: &str) -> Result<String> {
    let mut sanitized = path.replace('\\', "/"); // 统一分隔符

    // 移除危险字符
    let forbidden = ['<', '>', ':', '"', '|', '?', '*'];
    for c in &forbidden {
        sanitized = sanitized.replace(*c, "");
    }

    // 限制相对路径
    if sanitized.contains("..") {
        return Err("Invalid path".into());
    }

    // 标准化路径
    let path_buf = PathBuf::from(&sanitized)
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .collect::<PathBuf>();

    path_buf
        .to_str()
        .map(|s| s.to_string())
        .ok_or("Invalid path".into())
}

// 根据输入的字符串，生成8~14位数u32，不用哈希
pub fn generate_task_id(input: &str) -> u32 {
    let mut id = 0;
    for (i, c) in input.chars().enumerate() {
        id += (c as u32) * ((i + 1) as u32);
    }
    id % 1_000_000
}
