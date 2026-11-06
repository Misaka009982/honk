//! 从订阅入口到真实 TLS 与 AnyTLS 帧收发的回归覆盖，不依赖外部服务器。

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_rustls::{TlsAcceptor, rustls};

use super::super::super::{Subscription, parse_subscription_content};

async fn frame(stream: &mut (impl AsyncRead + Unpin)) -> (u8, u32, Vec<u8>) {
    let command = stream.read_u8().await.unwrap();
    let id = stream.read_u32().await.unwrap();
    let size = stream.read_u16().await.unwrap();
    let mut bytes = vec![0; usize::from(size)];
    stream.read_exact(&mut bytes).await.unwrap();
    (command, id, bytes)
}

async fn send(stream: &mut (impl AsyncWrite + Unpin), command: u8, id: u32, bytes: &[u8]) {
    stream.write_u8(command).await.unwrap();
    stream.write_u32(id).await.unwrap();
    stream.write_u16(bytes.len().try_into().unwrap()).await.unwrap();
    stream.write_all(bytes).await.unwrap();
    stream.flush().await.unwrap();
}

#[tokio::test]
async fn sub_store_anytls_nodes_dial_and_exchange_payload() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let cert = rcgen::generate_simple_self_signed(vec!["edge.test".into()]).unwrap();
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
        let mut config = rustls::ServerConfig::builder_with_provider(
            rustls::crypto::aws_lc_rs::default_provider().into(),
        )
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert.cert.into()], key.into())
        .unwrap();
        config.alpn_protocols = vec![b"h2".to_vec()];
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let uri = format!("anytls://p%40ss%3Aword%23%25@127.0.0.1:{port}?security=none&type=tcp&alpn=h2&sni=edge.test&allowInsecure=1&udp=1#edge");
        let bodies = [
            uri.clone(),
            base64::engine::general_purpose::STANDARD.encode(&uri),
            format!("proxies:\n  - {{name: edge, type: anytls, server: 127.0.0.1, port: {port}, password: 'p@ss:word#%', network: tcp, udp: true, sni: edge.test, alpn: [h2], skip-cert-verify: true}}"),
            format!(r#"[{{"name":"edge","type":"anytls","server":"127.0.0.1","port":{port},"password":"p@ss:word#%","network":"tcp","udp":true,"sni":"edge.test","alpn":["h2"],"skip-cert-verify":true}}]"#),
        ];
        let (done, mut received) = tokio::sync::mpsc::channel(1);
        let server = tokio::spawn(async move {
            for _ in 0..4 {
                let (tcp, _) = listener.accept().await.unwrap();
                let mut tls = acceptor.accept(tcp).await.unwrap();
                assert_eq!(tls.get_ref().1.server_name(), Some("edge.test"));
                assert_eq!(tls.get_ref().1.alpn_protocol(), Some(b"h2".as_slice()));
                let mut authentication = [0; 32];
                tls.read_exact(&mut authentication).await.unwrap();
                assert_eq!(
                    authentication.as_slice(),
                    &Sha256::digest(b"p@ss:word#%")[..]
                );
                let padding = tls.read_u16().await.unwrap();
                tls.read_exact(&mut vec![0; usize::from(padding)])
                    .await
                    .unwrap();
                let mut target_received = false;
                loop {
                    let (command, id, payload) = frame(&mut tls).await;
                    match command {
                        0 | 1 => {}
                        4 => send(&mut tls, 10, 0, b"v=2\n").await,
                        2 if !target_received => {
                            assert_eq!(payload, b"\x03\x0etarget.example\x01\xbb");
                            target_received = true;
                            send(&mut tls, 7, id, &[]).await;
                        }
                        2 => {
                            assert_eq!(payload, b"through-sub-store");
                            send(&mut tls, 2, id, &payload).await;
                            received.recv().await.unwrap();
                            break;
                        }
                        other => panic!("非预期 AnyTLS 帧：{other}"),
                    }
                }
            }
        });
        let registry = honk_outbound::proxy::ProxyRegistry::default_resolver().unwrap();
        for body in bodies {
            let nodes = parse_subscription_content(&Subscription::default(), &body).unwrap();
            assert_eq!(nodes.len(), 1);
            let node = &nodes[0];
            assert!((honk_outbound::descriptor::descriptor(node.protocol()).supports_udp)(node));
            let mut connection = registry
                .dial(
                    node,
                    "192.0.2.1:443".parse().unwrap(),
                    Some("target.example"),
                    Duration::from_secs(2),
                )
                .await
                .unwrap();
            connection
                .stream
                .write_all(b"through-sub-store")
                .await
                .unwrap();
            let mut echo = [0; 17];
            connection.stream.read_exact(&mut echo).await.unwrap();
            assert_eq!(&echo, b"through-sub-store");
            drop(connection);
            done.send(()).await.unwrap();
        }
        server.await.unwrap();
    })
    .await
    .expect("AnyTLS 订阅拨号与收发应在有限时间内完成");
}
