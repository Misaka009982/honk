//! URI 中影响实际拨号的 AnyTLS 字段，补入与结构化订阅相同的节点模型。

use std::borrow::Cow;

use honk_config::diagnostic::{DiagnosticSources, SettingPath};
use honk_config::error::{DetailedConfigError, ErrorCategory};
use honk_config::node::Node;
use honk_config::options::vocab::{coalesce_equal, packet_network, verification_text};

pub(super) fn apply(node: &mut Node, link: &str) -> Result<(), DetailedConfigError> {
    let Some(config) = node.anytls_mut() else {
        return Ok(());
    };
    let url = reqwest::Url::parse(link).map_err(|_| invalid("config", "AnyTLS URL 无效"))?;
    let pairs: Vec<_> = url.query_pairs().collect();
    for transport in claims(&pairs, &["type", "network"]) {
        if !matches!(transport, "" | "tcp") {
            return Err(invalid("transport", "AnyTLS 订阅只支持原生 TCP/TLS 承载"));
        }
    }
    let udp = coalesce_equal(
        claims(&pairs, &["udp", "udp-relay", "udp_relay"])
            .map(|value| verification_text(value).map(Some))
            .chain(claims(&pairs, &["anytls-network"]).map(packet_network)),
        "AnyTLS UDP 能力声明冲突",
    )
    .map_err(|reason| invalid("network", reason))?;
    if let Some(udp) = udp {
        config.network = Some(if udp { "tcp,udp" } else { "tcp" }.into());
    }
    let alpn = coalesce_equal(
        claims(&pairs, &["alpn"]).map(|value| {
            let names: Vec<String> = value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect();
            Ok(Some(names))
        }),
        "AnyTLS ALPN 声明冲突",
    )
    .map_err(|reason| invalid("tls_alpn", reason))?;
    if let Some(alpn) = alpn {
        config.tls.alpn = alpn;
    }
    for (keys, target, field) in [
        (
            &["idle_session_timeout", "idle-session-timeout"][..],
            &mut config.idle_session_timeout,
            "anytls_idle_session_timeout",
        ),
        (
            &["idle_session_check_interval", "idle-session-check-interval"][..],
            &mut config.idle_session_check_interval,
            "anytls_idle_session_check_interval",
        ),
    ] {
        if let Some(value) = coalesce_equal(
            claims(&pairs, keys).map(|value| {
                super::super::clash::parse_feed_duration_secs(&serde_yaml::Value::String(
                    value.into(),
                ))
                .map(Some)
            }),
            "AnyTLS 会话时长声明冲突",
        )
        .map_err(|reason| invalid(field, reason))?
        {
            *target = Some(value);
        }
    }
    if let Some(value) = coalesce_equal(
        claims(&pairs, &["min_idle_session", "min-idle-session"]).map(|value| {
            value
                .parse::<usize>()
                .map(Some)
                .map_err(|_| "AnyTLS 空闲会话数无效")
        }),
        "AnyTLS 空闲会话数声明冲突",
    )
    .map_err(|reason| invalid("anytls_min_idle_session", reason))?
    {
        config.min_idle_session = Some(value);
    }
    node.validate()
        .map_err(|_| invalid("anytls", "AnyTLS 连接参数未通过校验"))?;
    node.id = node.derive_id();
    Ok(())
}

fn claims<'a>(
    pairs: &'a [(Cow<'_, str>, Cow<'_, str>)],
    keys: &'a [&str],
) -> impl Iterator<Item = &'a str> {
    pairs
        .iter()
        .filter(move |(key, _)| keys.contains(&key.as_ref()))
        .map(|(_, value)| value.as_ref())
}

fn invalid(field: &'static str, message: &'static str) -> DetailedConfigError {
    DetailedConfigError::new(
        ErrorCategory::Validation,
        "invalid-config-value",
        DiagnosticSources::new(None).root(),
        SettingPath::new("nodes").field(field),
        message,
    )
}
