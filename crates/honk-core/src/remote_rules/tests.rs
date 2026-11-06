use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn remote_rules_download_then_restore_without_network() {
    let directory = tempfile::tempdir().unwrap();
    let entry = directory.path().join("config.dae");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/rules.dae", listener.local_addr().unwrap());
    std::fs::write(
        &entry,
        format!("include {{ '{url}' }}\nrouting {{ fallback: direct }}"),
    )
    .unwrap();
    let body = "routing { domain(suffix: example.com) -> block }";
    let server = tokio::spawn(async move {
        for response in [body, "routing { domain(suffix: example.com) -> absent }"] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                assert!(request.len() < 4096);
                request.push(socket.read_u8().await.unwrap());
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            );
            socket.write_all(head.as_bytes()).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    prepare(&entry).await.unwrap();
    let config = honk_config::Config::from_file(entry.to_str().unwrap()).unwrap();
    assert_eq!(config.routing.rules.len(), 1);
    assert_eq!(config.routing.rules[0].outbound.as_str(), "block");
    let cached = remote::cache_path(directory.path(), &url).unwrap();
    assert_eq!(std::fs::read_to_string(&cached).unwrap(), body);
    prepare(&entry).await.unwrap();
    server.await.unwrap();
    assert_eq!(std::fs::read_to_string(&cached).unwrap(), body);
    prepare(&entry).await.unwrap();
    assert_eq!(std::fs::read_to_string(&cached).unwrap(), body);
}

#[tokio::test]
async fn periodic_refresh_returns_only_changes_and_preserves_local_edits() {
    use std::os::unix::fs::MetadataExt as _;

    let directory = tempfile::tempdir().unwrap();
    let entry = directory.path().join("config.dae");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/rules.dae", listener.local_addr().unwrap());
    let original = format!("include {{ '{url}' }}\nrouting {{ fallback: direct }}");
    std::fs::write(&entry, &original).unwrap();
    let cached = remote::cache_path(directory.path(), &url).unwrap();
    let old = "routing { domain(suffix: example.com) -> direct }";
    let new = "routing { domain(suffix: example.com) -> block }";
    cache::replace(&cached, old.as_bytes()).unwrap();
    let (_, accepted) = load_local(&entry, &mut Vec::new()).unwrap();
    let before = std::fs::metadata(&cached).unwrap();
    let server = tokio::spawn(async move {
        for body in [
            old,
            new,
            "routing { domain(suffix: example.com) -> absent }",
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                assert!(request.len() < 4096);
                request.push(socket.read_u8().await.unwrap());
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(head.as_bytes()).await.unwrap();
            socket.write_all(body.as_bytes()).await.unwrap();
        }
    });
    assert!(refresh(&entry, Some(&accepted)).await.unwrap().is_none());
    assert_eq!(std::fs::metadata(&cached).unwrap().ino(), before.ino());
    let update = refresh(&entry, Some(&accepted)).await.unwrap().unwrap();
    assert_eq!(
        update.loaded.config.routing.rules[0].outbound.as_str(),
        "block"
    );
    assert!(
        refresh(&entry, Some(&update.loaded.sources))
            .await
            .unwrap()
            .is_none()
    );
    server.await.unwrap();
    assert!(
        refresh(&entry, Some(&update.loaded.sources))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(std::fs::read_to_string(&cached).unwrap(), new);
    std::fs::write(
        &entry,
        original.replace("fallback: direct", "fallback: block"),
    )
    .unwrap();
    let error = refresh(&entry, Some(&update.loaded.sources))
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("本地配置"));
    assert_eq!(std::fs::read_to_string(&cached).unwrap(), new);
}

#[tokio::test]
async fn local_edit_during_download_prevents_cache_publication() {
    let body = "routing { domain(suffix: remote.example) -> block }";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/rules.dae", listener.local_addr().unwrap());
    let directory = tempfile::tempdir().unwrap();
    let entry = directory.path().join("config.dae");
    let original = format!("include {{ '{url}' }}\nrouting {{ fallback: direct }}");
    std::fs::write(&entry, &original).unwrap();
    let cached = remote::cache_path(directory.path(), &url).unwrap();
    let old = "routing { domain(suffix: remote.example) -> direct }";
    cache::replace(&cached, old.as_bytes()).unwrap();
    let (_, accepted) = load_local(&entry, &mut Vec::new()).unwrap();
    let (received, wait) = tokio::sync::oneshot::channel();
    let (release, resume) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            assert!(request.len() < 4096);
            request.push(socket.read_u8().await.unwrap());
        }
        received.send(()).unwrap();
        resume.await.unwrap();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    let pending = refresh(&entry, Some(&accepted));
    tokio::pin!(pending);
    tokio::select! {
        _ = wait => {}
        _ = &mut pending => panic!("下载应等待服务端响应"),
    }
    std::fs::write(
        &entry,
        original.replace("fallback: direct", "fallback: block"),
    )
    .unwrap();
    release.send(()).unwrap();
    assert!(pending.await.is_err());
    server.await.unwrap();
    assert_eq!(std::fs::read_to_string(cached).unwrap(), old);
}

#[test]
fn cache_replacement_refuses_symlink_directory() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(
        outside.path(),
        directory.path().join(remote::CACHE_DIRECTORY),
    )
    .unwrap();
    let target = remote::cache_path(directory.path(), "https://example.com/rules.dae").unwrap();
    assert!(cache::replace(&target, b"routing { fallback: block }").is_err());
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn invalid_remote_configuration_cannot_replace_cached_rules() {
    for body in [
        "global { log_level: debug }",
        "include { 'other.dae' }",
        "routing { fallback: block }\nnode { 'socks5://127.0.0.1:1080' }",
        "routing { fallback: block }\nunknown { hidden: value }",
        "routing { domain(suffix: example.com) -> block",
    ] {
        assert!(remote::validate(body).is_err(), "{body}");
    }
    assert!(remote::validate("routing { domain(suffix: example.com) -> block }").is_ok());
}
