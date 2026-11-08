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
    let mut bootstrap = UnixStream::connect(&socket).unwrap();
    bootstrap
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let token = std::fs::read_to_string(directory.path().join("data/owner-token")).unwrap();
    bootstrap.write_all(format!("POST /api/v1/session HTTP/1.1\r\nHost: localhost\r\nOrigin: http://localhost\r\nAuthorization: Bearer {token}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
    let mut session = String::new();
    bootstrap.read_to_string(&mut session).unwrap();
    assert!(session.starts_with("HTTP/1.1 200"), "{session}");
    let cookie = session
        .lines()
        .find_map(|line| line.strip_prefix("set-cookie: "))
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let mut client = UnixStream::connect(&socket).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client.write_all(format!("GET /api/v1/accounts HTTP/1.1\r\nHost: localhost\r\nCookie: {cookie}\r\nx-colecto-extension: host\r\nx-colecto-account: demo\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("demo"));
    let mut events = UnixStream::connect(&socket).unwrap();
    events
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    events.write_all(format!("GET /api/v1/events HTTP/1.1\r\nHost: localhost\r\nCookie: {cookie}\r\nx-colecto-extension: host\r\nx-colecto-account: demo\r\n\r\n").as_bytes()).unwrap();
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

#[test]
fn local_owner_token_persists_privately_and_tightens_only_owned_data_directories() {
    use colecto_host::runtime::local_owner_token;
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let token = local_owner_token(directory.path()).unwrap();
    assert_eq!(token.len(), 64);
    assert_eq!(local_owner_token(directory.path()).unwrap(), token);
    assert_eq!(
        std::fs::metadata(directory.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let path = directory.path().join("owner-token");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(local_owner_token(directory.path()).is_err());
    std::fs::remove_file(&path).unwrap();
    let target = directory.path().join("unrelated");
    std::fs::write(&target, "keep this").unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert!(local_owner_token(directory.path()).is_err());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "keep this");
}

#[test]
fn native_owner_initialization_uses_storage_only_for_its_exact_local_origin() {
    let origin = "http://127.0.0.1:4173";
    let token = "test-token-with-quotes-\"-and-newline-\n";
    let script = colecto_host::runtime::owner_initialization_script(origin, token);
    let mut child = Command::new("node").arg("-e").arg(r#"
      const vm = require('node:vm');
      const assert = require('node:assert/strict');
      const input = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
      for (const origin of ['https://mail.example.test', 'https://127.0.0.1:4173', 'http://127.0.0.1:4174', 'http://evil.test']) {
        const window = { location: { origin }, get sessionStorage() { throw new Error('Remote storage accessed'); } };
        vm.runInNewContext(input.script, { window });
      }
      let writes = 0;
      const window = { location: { origin: input.origin }, sessionStorage: { setItem(key, value) { assert.equal(key, 'colecto.token'); assert.equal(value, input.token); writes++; } } };
      vm.runInNewContext(input.script, { window });
      assert.equal(writes, 1);
    "#).stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            serde_json::to_string(
                &serde_json::json!({"script":script,"origin":origin,"token":token}),
            )
            .unwrap()
            .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
