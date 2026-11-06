//! 定时规则下载通过真实协调器和路由发布，无需向进程发送 SIGHUP。

use super::*;
use honk_config::parser::remote;

const OLD_RULES: &str = "routing { domain(suffix: remote.example) -> direct }";
const NEW_RULES: &str = "routing { domain(suffix: remote.example) -> block }";

async fn fixture(listener: &TcpListener, interval: u64) -> Fixture {
    let url = format!("http://{}/rules.dae", listener.local_addr().unwrap());
    let fixture = Fixture::new_custom(Access::Admin, false, |root, files| {
        files.get_mut("main.dae").unwrap().push_str(&format!(
            "include {{ '{url}' }}\nassets {{ rules {{ interval: {interval}s }} }}\n"
        ));
        let path = remote::cache_path(root, &url).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, OLD_RULES).unwrap();
    })
    .await;
    fixture.barrier().await;
    fixture
}

async fn tick() {
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(61)).await;
    tokio::time::resume();
}

async fn request(listener: &TcpListener) -> TcpStream {
    timeout(WAIT, async {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            assert!(head.len() < 4096);
            head.push(socket.read_u8().await.unwrap());
        }
        socket
    })
    .await
    .unwrap()
}

async fn respond(mut socket: TcpStream, body: &str) {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).await.unwrap();
    socket.write_all(body.as_bytes()).await.unwrap();
}

async fn outbound(fixture: &Fixture) -> String {
    let trace = ok(fixture
        .request(Method::POST, "/api/v1/routing/trace")
        .json(&json!({
            "input":{"network":"tcp","domain":"remote.example","dst_ip":"192.0.2.1","dst_port":443},
            "resolve":"none"
        }))
        .send()
        .await
        .unwrap())
    .await;
    trace["evaluations"][0]["outbound"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn timer_applies_changed_rules_and_keeps_old_rules_after_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fixture = fixture(&listener, 60).await;
    let before = fixture.get(CONFIG).await;
    assert_eq!(outbound(&fixture).await, "direct");
    tick().await;
    respond(request(&listener).await, OLD_RULES).await;
    fixture.barrier().await;
    assert_eq!(fixture.reloads.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.get(CONFIG).await, before);

    tick().await;
    respond(request(&listener).await, NEW_RULES).await;
    fixture.barrier().await;
    assert_eq!(fixture.reloads.load(Ordering::SeqCst), 1);
    assert_eq!(outbound(&fixture).await, "block");
    let accepted = fixture.get(CONFIG).await;
    assert_ne!(accepted["revision"], before["revision"]);

    tick().await;
    respond(
        request(&listener).await,
        "routing { domain(suffix: remote.example) -> absent }",
    )
    .await;
    fixture.barrier().await;
    assert_eq!(fixture.reloads.load(Ordering::SeqCst), 1);
    assert_eq!(outbound(&fixture).await, "block");
    assert_eq!(fixture.get(CONFIG).await, accepted);
    fixture.shutdown().await;
}

#[tokio::test]
async fn manual_reload_queues_behind_periodic_refresh_and_shutdown_cancels_download() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut fixture = fixture(&listener, 60).await;
    tick().await;
    let first = request(&listener).await;
    fixture.service.request_sighup().unwrap();
    assert!(timeout(Duration::from_millis(30), listener.accept()).await.is_err());
    respond(first, OLD_RULES).await;
    let mut second = request(&listener).await;
    timeout(WAIT, fixture.coordinator.take().unwrap().shutdown())
        .await
        .unwrap();
    let mut byte = [0];
    let result = timeout(WAIT, second.read(&mut byte)).await.unwrap();
    assert!(matches!(result, Ok(0) | Err(_)));
    assert_eq!(fixture.reloads.load(Ordering::SeqCst), 0);
    fixture.shutdown().await;
}

#[tokio::test]
async fn shutdown_cancels_an_in_flight_periodic_download() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut fixture = fixture(&listener, 60).await;
    tick().await;
    let mut socket = request(&listener).await;
    timeout(WAIT, fixture.coordinator.take().unwrap().shutdown())
        .await
        .unwrap();
    let mut byte = [0];
    assert!(matches!(
        timeout(WAIT, socket.read(&mut byte)).await.unwrap(),
        Ok(0) | Err(_)
    ));
    tick().await;
    assert!(timeout(Duration::from_millis(30), listener.accept()).await.is_err());
    assert_eq!(fixture.reloads.load(Ordering::SeqCst), 0);
    assert_eq!(outbound(&fixture).await, "direct");
    fixture.shutdown().await;
}

#[tokio::test]
async fn disabled_timer_stays_off_until_explicit_reload_enables_it() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fixture = fixture(&listener, 0).await;
    tick().await;
    assert!(timeout(Duration::from_millis(30), listener.accept()).await.is_err());
    let entry = fixture.path("main.dae");
    let enabled = fixture.originals["main.dae"].replace("interval: 0s", "interval: 60s");
    std::fs::write(&entry, enabled).unwrap();
    fixture.service.request_sighup().unwrap();
    respond(request(&listener).await, OLD_RULES).await;
    fixture.barrier().await;
    assert_eq!(fixture.active.read().await.assets.rules.interval_secs(), 60);
    tick().await;
    respond(request(&listener).await, NEW_RULES).await;
    fixture.barrier().await;
    assert_eq!(outbound(&fixture).await, "block");
    fixture.shutdown().await;
}
