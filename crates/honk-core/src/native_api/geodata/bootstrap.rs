//! 首次启动补齐缺失的 Geo 文件；已有文件和运行中的更新仍由既有所有者管理。

use std::path::Path;
use std::sync::Arc;

use honk_config::Config;

use super::{Route, Sources};
use crate::dns::routing::DnsRouter;
use crate::routing::{GeoAssetSnapshot, GeoRequirements, GeoSourceSet};
use crate::state::StateDb;

pub(crate) async fn prepare(config: &Config, db: Option<Arc<StateDb>>) -> anyhow::Result<()> {
    let settings = &config.experimental.native_api;
    let requirements = GeoRequirements::for_traffic(&config.routing.rules)
        .union(&DnsRouter::geo_requirements(&config.dns));
    let missing: Vec<_> = [
        ("geosite", crate::routing::find_geosite_dat()),
        ("geoip", crate::routing::find_geoip_dat()),
    ]
    .into_iter()
    .filter(|(_, path)| path.is_none())
    .map(|(kind, _)| kind)
    .collect();
    if missing.is_empty()
        || (!settings.enabled || !settings.config_write)
            && missing
                .iter()
                .all(|kind| requirements.codes(kind).is_empty())
    {
        return Ok(());
    }
    let sources = match db {
        Some(db) => {
            let settings = settings.clone();
            Some(
                match tokio::task::spawn_blocking(move || Sources::open(db, &settings)).await? {
                    Ok(sources) => sources,
                    Err(error)
                        if missing
                            .iter()
                            .all(|kind| requirements.codes(kind).is_empty()) =>
                    {
                        crate::state::log_sql(&error);
                        tracing::warn!("Geo 设置不可用，跳过非必需预下载");
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                },
            )
        }
        None => None,
    };
    let route = super::route(settings, sources.as_ref());
    let direct = matches!(route, Route::Direct);
    let preload = settings.enabled
        && settings.config_write
        && sources
            .as_ref()
            .is_none_or(|sources| sources.effective().auto_update.enabled);
    for kind in missing {
        let codes = requirements.codes(kind);
        let required = !codes.is_empty();
        if !required && !preload {
            continue;
        }
        if !direct {
            anyhow::ensure!(
                !required,
                "缺少 {kind}.dat；首次自动下载需要 assets.geodata.route: direct，或预先提供本地 Geo 文件"
            );
            continue;
        }
        let urls = super::urls(settings, sources.as_ref(), kind);
        let verify = sources
            .as_ref()
            .is_none_or(|sources| sources.effective().verify_checksum);
        let result = fetch_and_install(
            kind,
            &urls,
            &config.global.bootstrap_resolver,
            verify,
            &codes,
            honk_config::paths::data_dir(),
        )
        .await;
        match result {
            Ok(()) => tracing::info!(asset = kind, "首次 Geo 数据已下载并校验"),
            Err(error) if required => return Err(error),
            Err(error) => tracing::warn!(
                asset = kind,
                %error,
                "Geo 预下载失败；使用 Geo 规则前需配置可达下载源并重启"
            ),
        }
    }
    Ok(())
}

async fn fetch_and_install(
    kind: &'static str,
    urls: &[String],
    bootstrap: &str,
    verify: bool,
    codes: &[String],
    directory: &Path,
) -> anyhow::Result<()> {
    let mut last = "invalid_source";
    for url in urls {
        let candidate = fetch_direct_asset(url, bootstrap, verify).await;
        let bytes = match candidate {
            Ok(bytes) => bytes,
            Err(error) => {
                last = error.code;
                continue;
            }
        };
        if validate(kind, &bytes, codes).is_err() {
            last = "asset_validation_failed";
            continue;
        }
        let path = directory.join(format!("{kind}.dat"));
        tokio::task::spawn_blocking(move || {
            crate::native_api::config_write::create_new(&path, &bytes, 0o600, || Ok(()))
        })
        .await??;
        return Ok(());
    }
    anyhow::bail!("首次 {kind} 下载失败：{last}；未安装不完整数据")
}

async fn fetch_direct_asset(
    url: &str,
    bootstrap: &str,
    verify: bool,
) -> Result<Arc<[u8]>, super::Failure> {
    let started = tokio::time::Instant::now();
    let bytes = super::download_direct(
        url,
        bootstrap,
        crate::marked_http::Deadline {
            headers: started + super::IDLE_TIMEOUT,
            idle: Some((super::IDLE_TIMEOUT, started + super::DOWNLOAD_LIMIT)),
        },
        crate::native_api::offline::MAX_ASSET_BYTES,
    )
    .await?;
    if verify {
        let mut checksum =
            honk_config::experimental::parse_geodata_url(url).ok_or("invalid_source")?;
        checksum.set_path(&format!("{}.sha256sum", checksum.path()));
        match super::download_direct(
            checksum.as_str(),
            bootstrap,
            (tokio::time::Instant::now() + super::CHECKSUM_TIMEOUT).into(),
            super::MAX_CHECKSUM_BYTES,
        )
        .await
        {
            Ok(published) => {
                let expected = std::str::from_utf8(&published)
                    .ok()
                    .and_then(|value| value.split_whitespace().next());
                if !expected.is_some_and(|expected| {
                    expected.eq_ignore_ascii_case(&crate::configuration::digest(&bytes))
                }) {
                    return Err("checksum_mismatch".into());
                }
            }
            Err(super::Failure {
                code: "http_not_found",
                ..
            }) => {}
            Err(error) => {
                return Err(super::Failure {
                    code: "checksum_unavailable",
                    status: error.status,
                });
            }
        }
    }
    Ok(bytes)
}

fn validate(kind: &'static str, bytes: &Arc<[u8]>, codes: &[String]) -> anyhow::Result<()> {
    anyhow::ensure!(!bytes.is_empty(), "Geo 数据为空");
    let mut requirements = GeoRequirements::default();
    for code in codes {
        match kind {
            "geosite" => requirements.add_geosite(code),
            "geoip" => requirements.add_geoip(code),
            _ => anyhow::bail!("未知 Geo 类型"),
        }
    }
    let snapshot = GeoAssetSnapshot {
        kind,
        path: None,
        sha256: crate::configuration::digest(bytes),
        size_bytes: bytes.len() as u64,
        modified_at: None,
    };
    GeoSourceSet::from_assets(&requirements, vec![(snapshot, Arc::clone(bytes))])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // 独立的 protobuf 样本：cn 分类包含 1.0.0.0/8。
    const GEOIP: &[u8] = b"\x0a\x0e\x0a\x02cn\x12\x08\x0a\x04\x01\x00\x00\x00\x10\x08";

    async fn origin(checksum: String, requests: usize) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/geoip-lite.dat", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for index in 0..requests {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    assert!(request.len() < 4096);
                    request.push(socket.read_u8().await.unwrap());
                }
                let body = if index % 2 == 0 {
                    assert!(request.starts_with(b"GET /geoip-lite.dat HTTP/1.1\r\n"));
                    GEOIP
                } else {
                    assert!(request.starts_with(b"GET /geoip-lite.dat.sha256sum HTTP/1.1\r\n"));
                    checksum.as_bytes()
                };
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(head.as_bytes()).await.unwrap();
                socket.write_all(body).await.unwrap();
            }
        });
        (url, server)
    }

    #[tokio::test]
    async fn bootstrap_installs_lite_as_geoip_and_never_overwrites_an_existing_file() {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let directory = tempfile::tempdir().unwrap();
            let (url, server) = origin(crate::configuration::digest(GEOIP), 4).await;
            let urls = [url];
            fetch_and_install("geoip", &urls, "", true, &["cn".into()], directory.path())
                .await
                .unwrap();
            let path = directory.path().join("geoip.dat");
            assert_eq!(std::fs::read(&path).unwrap(), GEOIP);
            assert!(!directory.path().join("geoip-lite.dat").exists());
            std::fs::write(&path, b"existing-file").unwrap();
            assert!(
                fetch_and_install("geoip", &urls, "", true, &["cn".into()], directory.path())
                    .await
                    .is_err()
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"existing-file");
            server.await.unwrap();
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn bootstrap_checksum_mismatch_leaves_no_asset() {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let directory = tempfile::tempdir().unwrap();
            let (url, server) = origin("0".repeat(64), 2).await;
            assert!(
                fetch_and_install("geoip", &[url], "", true, &["cn".into()], directory.path())
                    .await
                    .is_err()
            );
            assert!(!directory.path().join("geoip.dat").exists());
            server.await.unwrap();
        })
        .await
        .unwrap();
    }

    #[test]
    fn bootstrap_rejects_empty_malformed_and_missing_categories() {
        for bytes in [vec![], b"<!doctype html>error".to_vec(), vec![0x0a, 0]] {
            assert!(validate("geosite", &bytes.into(), &["cn".into()]).is_err());
        }
        let bytes = Arc::from(GEOIP);
        assert!(validate("geoip", &bytes, &["cn".into()]).is_ok());
        assert!(validate("geoip", &bytes, &["absent".into()]).is_err());
    }
}
