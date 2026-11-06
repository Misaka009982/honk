//! 远程规则下载与本地缓存；解析器、源管理与既有重载继续拥有配置语义。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use honk_config::diagnostic::DetailedDiagnostic;
use honk_config::parser::{LoadedConfig, SourceLimits, SourceSnapshot, remote};

use crate::marked_http;

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) mod schedule;

pub(crate) struct Update {
    pub(crate) loaded: LoadedConfig,
    pub(crate) diagnostics: Vec<DetailedDiagnostic>,
}

pub(crate) fn load_local(
    entry: &Path,
    diagnostics: &mut Vec<DetailedDiagnostic>,
) -> Result<(honk_config::Config, Vec<SourceSnapshot>), honk_config::error::DetailedConfigError> {
    if entry
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dae"))
    {
        let start = diagnostics.len();
        if let Ok(loaded) = honk_config::Config::from_dae_file_with_sources(
            entry,
            &HashMap::new(),
            SourceLimits::DEFAULT,
            diagnostics,
        ) {
            let config = crate::admit_operator_config(
                loaded.config,
                loaded.sources[0].source.clone(),
                diagnostics,
            )?;
            return Ok((config, loaded.sources));
        }
        diagnostics.truncate(start);
    }
    crate::load_operator_config(entry.to_str().unwrap_or(""), diagnostics)
        .map(|config| (config, Vec::new()))
}

pub(crate) fn has_remote(sources: &[SourceSnapshot]) -> bool {
    sources
        .first()
        .and_then(|source| source.path.parent())
        .is_some_and(|root| {
            let cache = root.join(remote::CACHE_DIRECTORY);
            sources.iter().any(|source| source.path.starts_with(&cache))
        })
}

pub(crate) async fn prepare(entry: &Path) -> anyhow::Result<()> {
    refresh(entry, None).await.map(|_| ())
}

/// 定时刷新只能在已接受的本地来源树上更新远程内容。
pub(crate) async fn refresh(
    entry: &Path,
    accepted: Option<&[SourceSnapshot]>,
) -> anyhow::Result<Option<Update>> {
    if !entry
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dae"))
    {
        return Ok(None);
    }
    // 非 dae 兼容正文和本地语法错误继续由原加载入口给出诊断。
    let Ok(local) = remote::local_sources(entry) else {
        return Ok(None);
    };
    if let Some(accepted) = accepted {
        check_local_sources(&local.sources, accepted)?;
    }
    let urls = remote::urls(&local.sources)?;
    if urls.is_empty() {
        return Ok(None);
    }
    anyhow::ensure!(
        local.sources.len().saturating_add(urls.len()) <= SourceLimits::DEFAULT.max_sources,
        "远程规则与本地配置合计超过来源上限"
    );
    honk_outbound::util::init_bypass_mark(local.config.global.effective_so_mark())?;
    let root = local.sources[0]
        .path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("配置入口缺少父目录"))?;
    let mut overlay: HashMap<_, Arc<str>> = local
        .sources
        .iter()
        .map(|source| (source.path.clone(), source.content.clone()))
        .collect();
    let mut fetched = Vec::new();
    let mut paths = Vec::new();
    let local_bytes = local
        .sources
        .iter()
        .map(|source| source.content.len())
        .sum::<usize>();
    let mut bytes = local_bytes;
    for url in urls {
        let path = remote::cache_path(root, &url)?;
        anyhow::ensure!(!overlay.contains_key(&path), "远程规则重复引用");
        let remaining = SourceLimits::DEFAULT.max_bytes.saturating_sub(bytes);
        let downloaded = download(&url, &local.config.global.bootstrap_resolver, remaining).await;
        let content = match downloaded.and_then(|body| {
            let text =
                std::str::from_utf8(&body).map_err(|_| anyhow::anyhow!("远程规则不是 UTF-8"))?;
            remote::validate(text)?;
            Ok(Arc::<str>::from(text))
        }) {
            Ok(content) => {
                fetched.push((path.clone(), content.clone()));
                content
            }
            Err(error) => {
                let content = cache::read(&path, remaining)?;
                remote::validate(&content)?;
                tracing::warn!(%error, "远程规则刷新失败，继续使用有效缓存");
                Arc::from(content)
            }
        };
        bytes = bytes
            .checked_add(content.len())
            .filter(|bytes| *bytes <= SourceLimits::DEFAULT.max_bytes)
            .ok_or_else(|| anyhow::anyhow!("远程规则与本地配置合计超过 8 MiB"))?;
        paths.push(path.clone());
        overlay.insert(path, content);
    }
    let update = match validate_candidate(&local.sources[0].path, &overlay) {
        Ok(update) => update,
        Err(error) => {
            let mut bytes = local_bytes;
            for path in &paths {
                overlay.remove(path);
            }
            fetched.clear();
            for path in paths {
                let content =
                    cache::read(&path, SourceLimits::DEFAULT.max_bytes.saturating_sub(bytes))?;
                remote::validate(&content)?;
                bytes += content.len();
                overlay.insert(path, Arc::from(content));
            }
            let update = validate_candidate(&local.sources[0].path, &overlay)?;
            tracing::warn!(%error, "远程规则候选配置被拒绝，保留上次有效缓存");
            update
        }
    };
    if accepted.is_some() {
        let current = remote::local_sources(entry)?;
        check_local_sources(&current.sources, &local.sources)?;
    }
    for (path, content) in fetched {
        if !cache::read(&path, SourceLimits::DEFAULT.max_bytes)
            .is_ok_and(|cached| cached == content.as_ref())
        {
            cache::replace(&path, content.as_bytes())?;
        }
    }
    if accepted.is_some_and(|accepted| same_sources(&update.loaded.sources, accepted)) {
        return Ok(None);
    }
    Ok(Some(update))
}

pub(crate) async fn refresh_active(
    entry: &Path,
    accepted: &mut Vec<SourceSnapshot>,
    activation: &mut crate::configuration::Activation,
) -> Result<(), crate::configuration::ActivationFailure> {
    let update = match refresh(entry, Some(accepted)).await {
        Ok(Some(update)) => update,
        Ok(None) => return Ok(()),
        Err(error) => {
            tracing::warn!(%error, "远程规则定时刷新失败，保留当前配置");
            return Ok(());
        }
    };
    let mut diagnostics = update.diagnostics;
    let mut candidate = match crate::admit_operator_config(
        update.loaded.config,
        update.loaded.sources[0].source.clone(),
        &mut diagnostics,
    ) {
        Ok(candidate) => candidate,
        Err(error) => {
            crate::report_runtime_admission_error(&error);
            return Ok(());
        }
    };
    candidate.ensure_builtin_nodes();
    honk_config::diagnostic::report_detailed_diagnostics(&diagnostics);
    let completion = activation
        .activate(crate::configuration::ActivationRequest {
            candidate,
            diagnostics,
            #[cfg(feature = "native-api")]
            sources: None,
            #[cfg(feature = "native-api")]
            expected_group_revision: None,
            #[cfg(feature = "native-api")]
            deferred_provider: None,
        })
        .await;
    if matches!(
        &completion,
        Ok(_) | Err(crate::configuration::ActivationFailure::Degraded(_))
    ) {
        *accepted = update.loaded.sources;
    }
    match completion {
        Ok(outcome) => {
            tracing::info!(generation = outcome.generation(), "远程规则定时刷新已生效");
            Ok(())
        }
        Err(failure) => {
            tracing::warn!(?failure, "远程规则定时激活未成功");
            Err(failure)
        }
    }
}

fn same_sources(left: &[SourceSnapshot], right: &[SourceSnapshot]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.path == right.path && left.content == right.content)
}

fn check_local_sources(
    local: &[SourceSnapshot],
    accepted: &[SourceSnapshot],
) -> anyhow::Result<()> {
    let root = local
        .first()
        .and_then(|source| source.path.parent())
        .ok_or_else(|| anyhow::anyhow!("配置入口缺少父目录"))?;
    let cache = root.join(remote::CACHE_DIRECTORY);
    let mut expected = accepted
        .iter()
        .filter(|source| !source.path.starts_with(&cache));
    anyhow::ensure!(
        local.iter().all(|source| {
            expected.next().is_some_and(|accepted| {
                source.path == accepted.path && source.content == accepted.content
            })
        }) && expected.next().is_none(),
        "本地配置尚有未重载的修改，跳过远程规则定时刷新"
    );
    Ok(())
}

fn validate_candidate(
    entry: &Path,
    overlay: &HashMap<PathBuf, Arc<str>>,
) -> anyhow::Result<Update> {
    let mut diagnostics = Vec::new();
    let loaded = honk_config::Config::from_dae_sources_in_memory(
        entry,
        overlay,
        SourceLimits::DEFAULT,
        &mut diagnostics,
    )?;
    loaded.config.validate()?;
    crate::routing::Router::from_config(&loaded.config.routing)?;
    Ok(Update {
        loaded,
        diagnostics,
    })
}

async fn download(url: &str, bootstrap: &str, max_bytes: usize) -> anyhow::Result<Arc<[u8]>> {
    let mut url = reqwest::Url::parse(url).map_err(|_| anyhow::anyhow!("远程规则 URL 无效"))?;
    let origin = url.clone();
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    for redirects in 0..=5 {
        let headers = http::HeaderMap::new();
        let request = crate::download_route::Request {
            url: &url,
            headers: &headers,
            wants_body: |status, _| status == http::StatusCode::OK,
            deadline: deadline.into(),
            max_bytes,
            bootstrap: Some(bootstrap),
        };
        let reply = crate::download_route::fetch_direct(&request)
            .await
            .map_err(|stage| anyhow::anyhow!("远程规则下载失败：{stage}"))?;
        if marked_http::followed_redirect(reply.status) {
            anyhow::ensure!(redirects < 5, "远程规则重定向超过 5 次");
            let location = reply
                .location
                .as_ref()
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| anyhow::anyhow!("远程规则重定向地址无效"))?;
            let next = url
                .join(location)
                .map_err(|_| anyhow::anyhow!("远程规则重定向地址无效"))?;
            anyhow::ensure!(
                remote::cache_path(Path::new("/"), next.as_str()).is_ok()
                    && crate::subscription::subscription_redirect_error(&origin, &next).is_none(),
                "远程规则重定向不符合安全限制"
            );
            url = next;
            continue;
        }
        anyhow::ensure!(
            reply.status == http::StatusCode::OK,
            "远程规则服务器返回 HTTP {}",
            reply.status.as_u16()
        );
        return Ok(reply.body);
    }
    unreachable!()
}

mod cache;
#[cfg(test)]
mod tests;
