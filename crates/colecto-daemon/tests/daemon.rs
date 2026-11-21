use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use colecto_core::{Accounts, Error, MailStore, SecretStore};
use colecto_daemon::{AccessPolicy, Daemon, MailService};
use std::sync::Arc;
use tower::ServiceExt;

struct NoSecrets;
impl SecretStore for NoSecrets {
    fn get(&self, _: &str) -> Result<String, Error> {
        Err(Error::Missing)
    }
    fn set(&self, _: &str, _: &str) -> Result<(), Error> {
        Err(Error::Missing)
    }
    fn delete(&self, _: &str) -> Result<(), Error> {
        Ok(())
    }
}

#[tokio::test]
async fn remote_access_requires_token_and_origin_match() {
    let dir = tempfile::tempdir().unwrap();
    let accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(NoSecrets)).unwrap();
    let service = MailService::new(accounts).unwrap();
    let daemon = Daemon::new(
        service,
        AccessPolicy::remote("https://mail.example.test", "a".repeat(64)).unwrap(),
        dir.path().join("assets"),
    )
    .unwrap();
    for (token, origin, expected) in [
        (None, "https://mail.example.test", StatusCode::UNAUTHORIZED),
        (
            Some("wrong"),
            "https://mail.example.test",
            StatusCode::UNAUTHORIZED,
        ),
        (
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            "https://mail.example.test",
            StatusCode::OK,
        ),
        (
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            "https://evil.test",
            StatusCode::FORBIDDEN,
        ),
    ] {
        let mut request = Request::builder()
            .uri("/api/v1/accounts")
            .header("host", "mail.example.test")
            .header("origin", origin)
            .header("x-colecto-account", "demo");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = daemon
            .router()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[test]
fn remote_config_rejects_weak_tokens_and_insecure_public_origins() {
    assert!(AccessPolicy::remote("http://mail.example.test", "a".repeat(64)).is_err());
    assert!(AccessPolicy::remote("https://mail.example.test/path", "a".repeat(64)).is_err());
    assert!(AccessPolicy::remote("https://mail.example.test", "short".into()).is_err());
}

#[cfg(unix)]
#[test]
fn socket_cleanup_preserves_existing_and_replacement_files() {
    use colecto_daemon::runtime::OwnedSocket;
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.path().join("mail.sock");
    std::fs::write(&path, "unrelated").unwrap();
    assert!(OwnedSocket::bind(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "unrelated");
    std::fs::remove_file(&path).unwrap();
    let socket = OwnedSocket::bind(&path).unwrap();
    assert!(OwnedSocket::bind(&path).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, "replacement").unwrap();
    drop(socket);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "replacement");
    std::fs::remove_file(&path).unwrap();
    let socket = OwnedSocket::bind(&path).unwrap();
    drop(socket);
    assert!(!path.exists());
}

#[tokio::test]
async fn stopped_background_worker_returns_without_polling() {
    let dir = tempfile::tempdir().unwrap();
    let accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(NoSecrets)).unwrap();
    let service = MailService::new(accounts).unwrap();
    let (_, stop) = tokio::sync::watch::channel(true);
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        service.run_background(std::time::Duration::from_secs(60), stop),
    )
    .await
    .unwrap();
}

#[test]
fn static_asset_roots_cannot_expose_private_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let accounts = Accounts::open(MailStore::open(&data).unwrap(), Arc::new(NoSecrets)).unwrap();
    let service = MailService::new(accounts).unwrap();
    assert!(
        Daemon::new(
            service,
            AccessPolicy::local_ephemeral("localhost".into()).unwrap(),
            dir.path().to_path_buf(),
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn unix_http_runtime_serves_requests_and_cleans_socket_on_sigterm() {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        os::unix::{fs::PermissionsExt, net::UnixStream},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = directory.path().join("mail.sock");
    let token_path = directory.path().join("token");
    let token = "b".repeat(64);
    std::fs::write(&token_path, &token).unwrap();
    std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600)).unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_colecto-daemon"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("COLECTO_") {
            command.env_remove(key);
        }
    }
    let data_dir = directory.path().join("data");
    let mut child = command
        .env("COLECTO_SOCKET", &socket)
        .env("COLECTO_DATA_DIR", &data_dir)
        .env("COLECTO_TOKEN_FILE", &token_path)
        .env("COLECTO_NOTIFICATIONS", "off")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut output = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut output)
        .unwrap();
    assert!(output.contains("HTTP socket"), "{output}");

    let mut version = UnixStream::connect(&socket).unwrap();
    version
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    version
        .write_all(b"GET /api/v1/version HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    version.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("\"name\":\"colecto\""), "{response}");

    let mut client = UnixStream::connect(&socket).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client
        .write_all(
            format!(
                "GET /api/v1/accounts HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nx-colecto-account: demo\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("demo"));

    // Confirm no owner-token sidecar file was created in data_dir.
    assert!(!data_dir.join("owner-token").exists());
    assert!(data_dir.join("colecto.db").exists());

    let mut events = UnixStream::connect(&socket).unwrap();
    events
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    events
        .write_all(
            format!(
                "GET /api/v1/events HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nx-colecto-account: demo\r\n\r\n"
            )
            .as_bytes(),
        )
        .unwrap();
    let mut events = BufReader::new(events);
    loop {
        let mut line = String::new();
        assert!(events.read_line(&mut line).unwrap() > 0);
        if line.contains("event: ready") {
            break;
        }
    }
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let mut tail = String::new();
    events
        .read_to_string(&mut tail)
        .expect("event stream must close on shutdown");
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(5) {
            child.kill().unwrap();
            panic!("Daemon failed to exit after SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    assert!(status.success());
    assert!(!socket.exists());
}

#[tokio::test]
async fn remote_mode_allows_authenticated_native_mutations_without_origin() {
    let dir = tempfile::tempdir().unwrap();
    let accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(NoSecrets)).unwrap();
    let service = MailService::new(accounts).unwrap();
    let token = "a".repeat(64);
    let daemon = Daemon::new(
        service,
        AccessPolicy::remote("https://mail.example.test", token.clone()).unwrap(),
        dir.path().join("assets"),
    )
    .unwrap();

    // Native client POST with valid bearer and no Origin header succeeds.
    let ok_req = Request::builder()
        .method("POST")
        .uri("/api/v1/session")
        .header("host", "mail.example.test")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        daemon.router().oneshot(ok_req).await.unwrap().status(),
        StatusCode::OK
    );

    // Cross-origin POST with valid bearer is still rejected.
    let bad_origin = Request::builder()
        .method("POST")
        .uri("/api/v1/session")
        .header("host", "mail.example.test")
        .header("origin", "https://evil.test")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        daemon.router().oneshot(bad_origin).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[cfg(unix)]
#[test]
fn default_local_runtime_prints_usable_ephemeral_token_without_sidecar_files() {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpStream,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let directory = tempfile::tempdir().unwrap();
    let data_dir = directory.path().join("data");

    let mut command = Command::new(env!("CARGO_BIN_EXE_colecto-daemon"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("COLECTO_") {
            command.env_remove(key);
        }
    }
    let mut child = command
        .env("COLECTO_DATA_DIR", &data_dir)
        .env("COLECTO_PORT", "0")
        .env("COLECTO_NOTIFICATIONS", "off")
        .env("COLECTO_UPDATE_CHECK", "off")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut address = None;
    let mut token = None;
    for _ in 0..3 {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0);
        if let Some(url) = line.trim().strip_prefix("Colecto: http://") {
            address = Some(url.to_owned());
        } else if let Some(tok) = line.trim().strip_prefix("Access token (ephemeral): ") {
            token = Some(tok.to_owned());
        }
    }
    let address = address.expect("expected listening address on stdout");
    let token = token.expect("expected ephemeral token on stdout");
    let (_, port) = address.rsplit_once(':').unwrap();

    // Connect using localhost:<port> and the printed ephemeral token.
    let mut client = TcpStream::connect(&address).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client
        .write_all(
            format!(
                "GET /api/v1/accounts HTTP/1.1\r\nHost: localhost:{port}\r\nAuthorization: Bearer {token}\r\nx-colecto-account: demo\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("demo"));
    assert!(!data_dir.join("owner-token").exists());

    assert!(
        Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(5) {
            child.kill().unwrap();
            panic!("Daemon failed to exit after SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    assert!(status.success());
}
