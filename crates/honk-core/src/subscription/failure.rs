//! 订阅失败只保留固定原因和数值位置，禁止把 URL、正文或凭据放入 API 错误。

use honk_config::diagnostic::DetailedDiagnostic;
use honk_config::error::DetailedConfigError;

#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("订阅下载失败：{stage}，HTTP 状态：{http_status:?}")]
pub(super) struct DownloadFailure {
    pub(super) stage: &'static str,
    pub(super) http_status: Option<u16>,
}

impl DownloadFailure {
    pub(super) fn stage(stage: &'static str) -> Self {
        Self {
            stage,
            http_status: None,
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub(crate) struct FailureSummary {
    pub(crate) stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entry_index: Option<usize>,
    pub(crate) reason: &'static str,
}

impl FailureSummary {
    pub(super) fn capture(error: &anyhow::Error, diagnostics: &[DetailedDiagnostic]) -> Self {
        let transport = error
            .downcast_ref::<DownloadFailure>()
            .copied()
            .or_else(|| {
                error
                    .downcast_ref::<crate::marked_http::Error>()
                    .map(|error| DownloadFailure::stage(error.stage))
            });
        if let Some(failure) = transport {
            return Self {
                stage: failure.stage,
                http_status: failure.http_status,
                diagnostic_code: None,
                entry_index: None,
                reason: match failure.stage {
                    "http_status_rejected" => "订阅服务器返回了错误的 HTTP 状态。",
                    "bootstrap_unavailable" => "订阅下载缺少可用的 bootstrap DNS 配置。",
                    "resolution_failed" => "订阅服务器域名解析失败，请检查 DNS 配置。",
                    "tls_failed" => "订阅 TLS 握手或证书校验失败。",
                    "download_timeout" => "订阅下载超时，请检查下载出口与订阅服务器。",
                    "content_encoding_rejected" => "订阅服务器未按请求返回未压缩正文。",
                    "asset_too_large" => "订阅正文超过 8 MiB 限制。",
                    "route_blocked" => "路由规则阻止了订阅下载。",
                    "redirect_rejected" => "订阅重定向不符合 HTTPS 与地址范围限制。",
                    "too_many_redirects" => "订阅重定向超过 5 次。",
                    _ => "订阅传输失败，请检查 DNS、TLS 与下载出口。",
                },
            };
        }
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                matches!(
                    diagnostic.code,
                    "malformed-subscription-entry" | "unsupported-subscription-entry"
                )
            })
            .or_else(|| {
                error
                    .downcast_ref::<DetailedConfigError>()
                    .map(|error| error.diagnostic.as_ref())
            });
        if let Some(diagnostic) = diagnostic {
            return Self {
                stage: if diagnostic.code == "invalid-subscription-encoding" {
                    "body_encoding"
                } else {
                    "body_parse"
                },
                http_status: None,
                diagnostic_code: Some(diagnostic.code),
                entry_index: diagnostic.entry_index,
                reason: diagnostic.message,
            };
        }
        Self {
            stage: "connection_failed",
            http_status: None,
            diagnostic_code: None,
            entry_index: None,
            reason: "订阅连接失败，请检查 DNS、TLS 与下载出口；详细原因见引擎日志。",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_errors_never_expose_their_payload() {
        let summary = FailureSummary::capture(
            &anyhow::anyhow!("https://user:secret@example.com/sub?token=PRIVATE_TOKEN"),
            &[],
        );
        let rendered = serde_json::to_string(&summary).unwrap();
        for secret in ["secret", "example.com", "PRIVATE_TOKEN"] {
            assert!(!rendered.contains(secret));
        }
        assert_eq!(summary.stage, "connection_failed");
    }

    #[test]
    fn transport_failure_retains_only_stage_and_status() {
        let error = anyhow::Error::new(DownloadFailure {
            stage: "http_status_rejected",
            http_status: Some(403),
        });
        let summary = FailureSummary::capture(&error, &[]);
        assert_eq!(summary.stage, "http_status_rejected");
        assert_eq!(summary.http_status, Some(403));
        assert!(summary.diagnostic_code.is_none());
    }
}
