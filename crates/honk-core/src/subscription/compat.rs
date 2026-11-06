//! 订阅格式适配集中在入口，避免改变原生节点与其他导入器的语义。

use std::borrow::Cow;

use honk_config::diagnostic::{DetailedDiagnostic, DiagnosticSources, SafeValue, SettingPath};
use honk_config::error::DetailedConfigError;
use honk_config::node::Node;
use serde_yaml::{Mapping, Value};

mod anytls;

pub(super) fn parse_share_link(
    link: &str,
    emit: &mut impl FnMut(DetailedDiagnostic),
) -> Result<Node, DetailedConfigError> {
    let normalized = share_link(link);
    let mut node = Node::from_share_link_with_detailed_diagnostics_emit(
        normalized.as_deref().unwrap_or(link),
        emit,
    )?;
    anytls::apply(&mut node, link)?;
    if normalized.is_some() {
        emit(DetailedDiagnostic::warning(
            "legacy-subscription-anytls-tls",
            DiagnosticSources::new(None).root(),
            SettingPath::new("nodes").field("tls"),
            SafeValue::Redacted,
            "订阅转换器的 AnyTLS security=none&type=tcp 已按强制 TLS 导入；证书校验设置保持不变",
        ));
    }
    Ok(node)
}

fn share_link(link: &str) -> Option<String> {
    if !link
        .get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("anytls://"))
    {
        return None;
    }
    let mut url = reqwest::Url::parse(link).ok()?;
    let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    let values = |key: &'static str| {
        pairs
            .iter()
            .filter(move |(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    // Sub-Store 复用 VLESS URI 输出器，会给强制 TLS 的 AnyTLS 附加此默认值。
    // 明确的 tls 或 REALITY 声明仍交给原生解析器检查，不能掩盖相互矛盾的选项。
    if !values("security").any(|value| value == "none")
        || values("security").any(|value| value != "none")
        || !values("type").any(|value| value == "tcp")
        || values("type").any(|value| value != "tcp")
        || values("network").any(|value| !matches!(value, "" | "tcp"))
        || ["tls", "pbk", "sid", "spx"]
            .iter()
            .any(|key| values(key).next().is_some())
    {
        return None;
    }
    url.query_pairs_mut()
        .clear()
        .extend_pairs(pairs.iter().map(|(key, value)| {
            (
                key.as_str(),
                if key == "security" {
                    "tls"
                } else {
                    value.as_str()
                },
            )
        }));
    Some(url.into())
}

pub(super) fn clash_list(value: &Value) -> Option<&[Value]> {
    let Value::Sequence(entries) = value else {
        return None;
    };
    entries
        .iter()
        .any(|entry| {
            entry.as_mapping().is_some_and(|entry| {
                entry.contains_key("type")
                    && entry.contains_key("server")
                    && entry.contains_key("port")
            })
        })
        .then_some(entries)
}

pub(super) fn clash_proxy(mapping: &Mapping) -> Cow<'_, Mapping> {
    let text = |key| mapping.get(key).and_then(Value::as_str);
    if text("type").is_some_and(|value| value.eq_ignore_ascii_case("anytls"))
        && text("network") == Some("tcp")
    {
        // Clash 的 tcp 在此描述承载传输；显式 anytls-network 仍保留能力限制。
        let mut normalized = mapping.clone();
        normalized.remove("network");
        Cow::Owned(normalized)
    } else {
        Cow::Borrowed(mapping)
    }
}

#[cfg(test)]
mod tests;
