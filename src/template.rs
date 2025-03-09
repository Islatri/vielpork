use crate::error::Result;
use crate::base::structs::DownloadMeta;
use handlebars::Handlebars;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::Path;

pub struct TemplateRenderer {
    registry: Handlebars<'static>,
}

impl TemplateRenderer {
    pub fn new() -> Self {
        let mut registry = Handlebars::new();
        registry.register_escape_fn(|s| s.into()); // 禁用HTML转义
        
        // 注册自定义helper
        registry.register_helper("date_format", Box::new(date_format_helper));
        registry.register_helper("file_extension", Box::new(file_extension_helper));

        Self { registry }
    }

    /// 渲染路径模板
    pub fn render_path_template(
        &self,
        template: &str,
        context: &TemplateContext,
    ) -> Result<String> {
        let mut data = serde_json::json!({
            "url": context.url,
            "domain": context.domain,
            "filename": context.filename,
            "ext": context.extension,
            "size": context.meta.expected_size,
            "content_type": context.meta.content_type,
            "date": context.download_time.format("%Y-%m-%d").to_string(),
            "time": context.download_time.format("%H-%M-%S").to_string(),
        });

        // 添加自定义元数据
        if let Some(ref custom) = context.custom_data {
            for (k, v) in custom {
                data[k] = serde_json::Value::String(v.clone());
            }
        }

        self.registry
            .render_template(template, &data)
            .map_err(|e| e.into())
    }
}

/// 模板上下文数据
pub struct TemplateContext<'a> {
    pub url: &'a str,
    pub domain: Option<&'a str>,
    pub filename: &'a str,
    pub extension: Option<&'a str>,
    pub meta: &'a DownloadMeta,
    pub download_time: DateTime<Utc>,
    pub custom_data: Option<HashMap<String, String>>,
}

/// 自定义日期格式化helper
fn date_format_helper(
    h: &handlebars::Helper<'_>,
    _: &Handlebars<'_>,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext<'_,'_>,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let format = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("%Y-%m-%d");
    let timestamp = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    
    let dt = DateTime::parse_from_rfc3339(timestamp)
        .or_else(|_| DateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S")).unwrap_or_default();
    
    out.write(&dt.format(format).to_string())?;
    Ok(())
}

/// 文件扩展名提取helper
fn file_extension_helper(
    h: &handlebars::Helper<'_>,
    _: &Handlebars<'_>,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext<'_,'_>,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let filename = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    
    out.write(ext)?;
    Ok(())
}
