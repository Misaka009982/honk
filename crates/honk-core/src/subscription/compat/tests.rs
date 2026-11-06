use super::super::*;
use base64::Engine as _;

mod anytls_wire;

#[test]
fn sub_store_anytls_uri_keeps_tls_credentials_and_identity() {
    let canonical = "anytls://p%40ss%3Aword%23%25@example.com:443?security=tls&type=tcp&packetEncoding=none&udp=1&sni=tls.example&idle_session_timeout=60#edge";
    let converted = canonical.replace("security=tls", "security=none");
    let mut expected = Node::from_share_link(canonical).unwrap();
    expected.anytls_mut().unwrap().network = Some("tcp,udp".into());
    expected.id = expected.derive_id();
    assert!(Node::from_share_link(&converted).is_err());
    for body in [
        converted.clone(),
        base64::engine::general_purpose::STANDARD.encode(&converted),
    ] {
        let mut diagnostics = Vec::new();
        let nodes = parse_subscription_content_with_diagnostics(
            &Subscription::default(),
            &body,
            &mut diagnostics,
        )
        .unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, expected.id);
        let anytls = nodes[0].anytls().unwrap();
        assert_eq!(anytls.password.as_deref(), Some("p@ss:word#%"));
        assert!(anytls.tls.enabled);
        assert!(!anytls.tls.skip_cert_verify);
        assert_eq!(anytls.tls.sni.as_deref(), Some("tls.example"));
        assert_eq!(anytls.idle_session_timeout, Some(60));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "legacy-subscription-anytls-tls");
        assert_eq!(diagnostics[0].entry_index, Some(1));
    }
}

#[test]
fn sub_store_anytls_compatibility_preserves_explicit_security_rejections() {
    for query in [
        "security=none",
        "security=none&type=tcp&tls=0",
        "security=none&type=tcp&tls=1",
        "security=none&type=tcp&security=tls",
        "security=none&type=tcp&pbk=secret",
        "security=none&type=ws",
        "security=none&type=tcp&type=ws",
        "security=none&type=tcp&network=ws",
    ] {
        let body = format!(
            "anytls://password@example.com:443?{query}\nsocks5://127.0.0.1:1080#retained"
        );
        let nodes = parse_subscription_content(&Subscription::default(), &body).unwrap();
        assert_eq!(nodes.len(), 1, "{query}");
        assert_eq!(nodes[0].name, "retained", "{query}");
    }
    assert!(super::share_link("trojan://password@example.com:443?security=none&type=tcp").is_none());
}

#[test]
fn sub_store_anytls_clash_transport_does_not_disable_udp() {
    let body = r#"proxies:
  - {name: converted, type: anytls, server: example.com, port: 443, password: secret, network: tcp, udp: true}
  - {name: duplicate, type: anytls, server: example.com, port: 443, password: secret, udp: true}
  - {name: restricted, type: anytls, server: restricted.example, port: 443, password: secret, network: tcp, udp: false}
  - {name: conflicting, type: anytls, server: conflict.example, port: 443, password: secret, network: tcp, anytls-network: tcp, udp: true}
  - {name: unsupported, type: anytls, server: unsupported.example, port: 443, password: secret, network: quic, udp: true}
  - {name: default-udp, type: anytls, server: default.example, port: 443, password: secret, network: tcp}
"#;
    let mut diagnostics = Vec::new();
    let nodes = parse_subscription_content_with_diagnostics(
        &Subscription::default(),
        body,
        &mut diagnostics,
    )
    .unwrap();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].name, "converted");
    assert_eq!(nodes[0].anytls().unwrap().network.as_deref(), Some("tcp,udp"));
    assert_eq!(nodes[1].name, "restricted");
    assert_eq!(nodes[1].anytls().unwrap().network.as_deref(), Some("tcp"));
    assert!(
        (honk_outbound::descriptor::descriptor(nodes[2].protocol()).supports_udp)(&nodes[2])
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "duplicate-subscription-entry" && diagnostic.entry_index == Some(2)
    }));
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.entry_index == Some(4)));
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.entry_index == Some(5)));
}

#[test]
fn anytls_uri_applies_alpn_and_udp_to_the_runtime_node() {
    let body = "anytls://secret@example.com:443?security=tls&type=tcp&alpn=h2%2Chttp%2F1.1&udp=0&idle-session-timeout=30s&min-idle-session=2";
    let nodes = parse_subscription_content(&Subscription::default(), body).unwrap();
    let node = &nodes[0];
    let config = node.anytls().unwrap();
    assert_eq!(config.tls.alpn, ["h2", "http/1.1"]);
    assert_eq!(config.network.as_deref(), Some("tcp"));
    assert_eq!(config.idle_session_timeout, Some(30));
    assert_eq!(config.min_idle_session, Some(2));
    assert_eq!(node.id, node.derive_id());
    assert!(!(honk_outbound::descriptor::descriptor(node.protocol()).supports_udp)(node));
    let registry = honk_outbound::proxy::ProxyRegistry::default_resolver().unwrap();
    assert!(registry.find(node.protocol()).is_some());
    for suffix in ["&udp=1", "&alpn=h3", "&type=ws", "&idle_session_timeout=10s"] {
        assert!(parse_subscription_content(&Subscription::default(), &format!("{body}{suffix}")).is_err());
    }
}

#[test]
fn sub_store_plain_json_preserves_supported_siblings() {
    let body = r#"[{"type":"anytls","name":"json","server":"example.com","port":443,"password":"secret","network":"tcp","udp":true},{"type":"unsupported","server":"other.example","port":443}]"#;
    for sub_type in [SubscriptionType::Simple, SubscriptionType::Clash] {
        let subscription = Subscription {
            sub_type,
            ..Default::default()
        };
        let nodes = parse_subscription_content(&subscription, body).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "json");
        assert_eq!(nodes[0].anytls().unwrap().network.as_deref(), Some("tcp,udp"));
    }
    let sip008 = r#"[{"server":"example.com","server_port":443,"password":"secret","method":"aes-128-gcm"}]"#;
    let nodes = parse_subscription_content(&Subscription::default(), sip008).unwrap();
    assert!(nodes[0].shadowsocks().is_some());
}

#[test]
fn sing_box_anytls_packet_restrictions_keep_their_source_meaning() {
    let body = r#"{"outbounds":[{"type":"anytls","tag":"tcp-only","server":"example.com","server_port":443,"password":"secret","network":"tcp","tls":{"enabled":true}}]}"#;
    let nodes = parse_subscription_content(&Subscription::default(), body).unwrap();
    assert_eq!(nodes[0].anytls().unwrap().network.as_deref(), Some("tcp"));
}
