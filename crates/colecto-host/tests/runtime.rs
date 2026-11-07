#![cfg(unix)]
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn unix_http_runtime_serves_requests_and_cleans_socket_on_sigterm() {
    use std::os::unix::{fs::PermissionsExt, net::UnixStream};
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = directory.path().join("mail.sock");
    let mut command = Command::new(env!("CARGO_BIN_EXE_colecto-daemon"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("COLECTO_") {
            command.env_remove(key);
        }
    }
    let mut child = command
        .env("COLECTO_SOCKET", &socket)
        .env("COLECTO_DATA_DIR", directory.path().join("data"))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut output = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut output)
        .unwrap();
    assert!(output.contains("HTTP socket"), "{output}");
    let mut client = UnixStream::connect(&socket).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client.write_all(b"GET /api/v1/accounts HTTP/1.1\r\nHost: localhost\r\nx-colecto-extension: host\r\nx-colecto-account: demo\r\nConnection: close\r\n\r\n").unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("demo"));
    let mut events = UnixStream::connect(&socket).unwrap();
    events
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    events.write_all(b"GET /api/v1/events HTTP/1.1\r\nHost: localhost\r\nx-colecto-extension: host\r\nx-colecto-account: demo\r\n\r\n").unwrap();
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
    assert!(
        !socket.exists(),
        "owned socket must be removed on graceful shutdown"
    );
}
