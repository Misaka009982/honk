//! 远程 include 的地址、缓存命名和规则边界；此模块不执行网络请求。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::cursor::Document;
use super::diagnostics::ParserDiagnostics;
use super::lexer::Source;
use super::{LoadedConfig, SourceLimits, SourceSnapshot};
use crate::diagnostic::{DetailedDiagnostic, DiagnosticSources, finish_attempt};
use crate::error::DetailedConfigError;

pub const CACHE_DIRECTORY: &str = ".honk-remote-rules";

#[derive(Clone, Copy)]
pub(super) enum LoadMode {
    Disk,
    Memory,
    Local,
}

pub fn is_remote(pattern: &str) -> bool {
    pattern
        .split_once("://")
        .is_some_and(|(scheme, _)| {
            scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http")
        })
}

pub fn cache_path(root: &Path, pattern: &str) -> Result<PathBuf, crate::ConfigError> {
    let url = url::Url::parse(pattern)
        .map_err(|_| crate::ConfigError::Include("远程规则 URL 无效".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(crate::ConfigError::Include(
            "远程规则要求不含用户信息或片段的 HTTP(S) URL".into(),
        ));
    }
    let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, url.as_str().as_bytes());
    Ok(root.join(CACHE_DIRECTORY).join(format!("{id}.dae")))
}

/// 只为下载准备读取本地配置；正常校验必须再次加载包含远程缓存的完整来源树。
pub fn local_sources(path: &Path) -> Result<LoadedConfig, DetailedConfigError> {
    let mut diagnostics = Vec::new();
    let source = DiagnosticSources::new(Some(path.to_owned())).root();
    let mut sink = ParserDiagnostics::new(&mut diagnostics, source);
    let result = super::parse_dae_file_inner(
        path,
        &HashMap::new(),
        SourceLimits::DEFAULT,
        None,
        LoadMode::Local,
        &mut sink,
        &mut false,
    )
    .map_err(|error| match error {
        super::ParseFailure::Detailed(error) => error,
        super::ParseFailure::Legacy(error) => sink.error(error),
    });
    sink.finish();
    result
}

pub fn urls(sources: &[SourceSnapshot]) -> Result<Vec<String>, crate::ConfigError> {
    let mut urls = Vec::new();
    for source in sources {
        let mut diagnostics = Vec::new();
        let document = Document::parse_attempt(
            Source::new(&source.content, source.source.clone()),
            &mut diagnostics,
            false,
        )
        .map_err(|error| error.error.into_legacy())?;
        for segment in document
            .sections()
            .filter(|segment| segment.header() == "include")
        {
            for pattern in super::parse_include_body(segment, &source.path)? {
                if is_remote(&pattern) {
                    urls.push(pattern);
                }
            }
        }
    }
    Ok(urls)
}

pub fn validate(content: &str) -> Result<(), DetailedConfigError> {
    let source = DiagnosticSources::new(None).root();
    let mut diagnostics: Vec<DetailedDiagnostic> = Vec::new();
    let result = (|| {
        let document = Document::parse(Source::new(content, source.clone()), &mut diagnostics)
            .map_err(|error| error.error)?;
        if !diagnostics.is_empty()
            || document
                .sections()
                .any(|segment| segment.header() != "routing")
            || !document
                .sections()
                .any(|segment| segment.header() == "routing")
        {
            return Err(super::sources::source_error(
                source,
                "invalid-remote-rules",
                "远程规则文件只能包含 routing 块，不能包含 include、节点、DNS 或全局设置",
            ));
        }
        super::parse_dae_config_with_detailed_diagnostics(content, &mut diagnostics)?;
        if !diagnostics.is_empty() {
            return Err(super::sources::source_error(
                document.source().reference(),
                "invalid-remote-rules",
                "远程规则包含不能完整识别的内容",
            ));
        }
        Ok(())
    })();
    finish_attempt(result, &mut diagnostics)
}
