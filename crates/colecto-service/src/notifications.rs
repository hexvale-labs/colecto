//! Desktop notifications for new mail while no Colecto window is open. The daemon runs this
//! against its own socket, or, in relay mode, against a remote daemon that does the syncing.
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

pub fn read_token(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Err("Token path must be a regular file, not a symlink".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != rustix::process::geteuid().as_raw() {
            return Err("Token file must be owned by the current OS user".into());
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("Token file must be readable only by its owner (chmod 600)".into());
        }
    }
    let token = std::fs::read_to_string(path)?.trim().to_owned();
    Ok(token)
}

fn connection(
    origin: &str,
) -> Result<(reqwest::Url, reqwest::blocking::Client), Box<dyn std::error::Error>> {
    let socket = origin.strip_prefix("unix:");
    if socket.is_some_and(|p| !Path::new(p).is_absolute()) {
        return Err("Socket path must be absolute".into());
    }
    let url = reqwest::Url::parse(if socket.is_some() {
        "http://localhost"
    } else {
        origin
    })?;
    if socket.is_none()
        && (url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some())
    {
        return Err("Notification agent requires an HTTPS server origin".into());
    }
    let builder = reqwest::blocking::Client::builder();
    #[cfg(unix)]
    let builder = if let Some(socket) = socket {
        builder.unix_socket(socket)
    } else {
        builder
    };
    #[cfg(not(unix))]
    if socket.is_some() {
        return Err("Unix sockets unavailable".into());
    }
    let client = builder
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    Ok((url, client))
}

pub fn run_agent(origin: &str, token_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (url, client) = connection(origin)?;
    let mut backoff = 1;
    loop {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let token = zeroize::Zeroizing::new(read_token(token_file)?);
            let response = client
                .get(url.join("api/v1/events")?)
                .bearer_auth(token.as_str())
                .header("x-colecto-account", "demo")
                .send()?
                .error_for_status()?;
            backoff = 1;
            consume_events(BufReader::new(response), || {
                if !mail_enabled(&client, &url, &token) {
                    return;
                }
                if let Err(error) = show("New mail", "Open Colecto to read your messages.") {
                    tracing::warn!(%error, "Desktop notification failed");
                }
            })?;
            Ok(())
        })();
        if let Err(error) = result {
            tracing::warn!(%error, retry_seconds = backoff, "Notification connection unavailable");
        }
        std::thread::sleep(Duration::from_secs(backoff));
        backoff = (backoff * 2).min(60);
    }
}

/// Notifications enabled preference; on when it cannot be read.
fn mail_enabled(client: &reqwest::blocking::Client, url: &reqwest::Url, token: &str) -> bool {
    let preferences = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(client
            .get(url.join("api/v1/preferences")?)
            .bearer_auth(token)
            .header("x-colecto-account", "demo")
            .timeout(Duration::from_secs(10))
            .send()?
            .error_for_status()?
            .json()?)
    })();
    preferences.map_or(true, |prefs| prefs["notifications"]["enabled"] != false)
}

/// Post a notification over the freedesktop D-Bus interface, which every Linux desktop
/// implements, rather than through `notify-send`, which not every distribution installs.
#[cfg(target_os = "linux")]
fn show(summary: &str, body: &str) -> Result<(), Box<dyn std::error::Error>> {
    use dbus::arg::{PropMap, Variant};
    let connection = dbus::blocking::Connection::new_session()?;
    let proxy = connection.with_proxy(
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        Duration::from_secs(5),
    );
    let mut hints = PropMap::new();
    hints.insert(
        "desktop-entry".into(),
        Variant(Box::new("colecto".to_owned())),
    );
    let actions: Vec<String> = Vec::new();
    let _: (u32,) = proxy.method_call(
        "org.freedesktop.Notifications",
        "Notify",
        (
            "Colecto", 0u32, "colecto", summary, body, actions, hints, -1i32,
        ),
    )?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn show(_: &str, _: &str) -> Result<(), Box<dyn std::error::Error>> {
    Err("Background desktop notifications are available only on Linux".into())
}

/// Calls `notify` for each mail event with new messages, unless a Colecto window was open
/// when it was sent: that window shows its own notification.
fn consume_events(reader: impl BufRead, mut notify: impl FnMut()) -> Result<(), std::io::Error> {
    let mut reader = reader;
    let mut event = String::new();
    let mut data = String::new();
    loop {
        let mut line = Vec::new();
        let count = std::io::Read::take(&mut reader, 65537).read_until(b'\n', &mut line)?;
        if count == 0 {
            return Ok(());
        }
        if count > 65536 {
            return Err(std::io::Error::other(
                "Notification event line exceeds limit",
            ));
        }
        let line = std::str::from_utf8(&line)
            .map_err(std::io::Error::other)?
            .trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if event == "mail"
                && serde_json::from_str::<serde_json::Value>(&data)
                    .ok()
                    .is_some_and(|v| {
                        v["ids"].as_array().is_some_and(|a| !a.is_empty())
                            && v["windows"].as_u64().unwrap_or(0) == 0
                    })
            {
                notify();
            }
            event.clear();
            data.clear();
        } else if let Some(value) = line.strip_prefix("event:") {
            event = value.trim_start().into();
        } else if let Some(value) = line.strip_prefix("data:") {
            if data.len() + value.len() > 65536 {
                return Err(std::io::Error::other("Notification event exceeds limit"));
            }
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim_start());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_ignore_initial_snapshot_and_empty_or_malformed_events() {
        let input = b"event: ready\ndata: {}\n\nevent: mail\ndata: {\"ids\":[]}\n\nevent: mail\ndata: invalid\n\nevent: mail\ndata: {\"account\":\"a\",\"ids\":[\"new\"]}\n\nevent: mail\ndata: {\"account\":\"a\",\"ids\":[\"open\"],\"windows\":1}\n\n";
        let mut count = 0;
        consume_events(&input[..], || count += 1).unwrap();
        assert_eq!(count, 1, "a window that is open shows its own notification");
    }

    #[test]
    fn oversized_events_are_bounded() {
        assert!(consume_events(&vec![b'x'; 65538][..], || {}).is_err());
    }
}

#[cfg(all(test, unix))]
mod socket_tests {
    use super::*;

    #[test]
    fn notifier_connects_over_unix_socket_with_localhost_authority() {
        use std::{io::Write, os::unix::net::UnixListener};
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("mail.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = String::new();
            let mut reader = BufReader::new(&mut stream);
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                request.push_str(&line);
            }
            assert!(request.starts_with("GET /api/v1/events HTTP/1.1"));
            assert!(request.to_ascii_lowercase().contains("host: localhost"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .unwrap();
        });
        let (url, client) = connection(&format!("unix:{}", socket.display())).unwrap();
        assert_eq!(
            client
                .get(url.join("api/v1/events").unwrap())
                .send()
                .unwrap()
                .text()
                .unwrap(),
            "ok"
        );
        server.join().unwrap();
        assert!(connection("unix:relative.sock").is_err());
        assert!(connection("http://example.com").is_err());
    }
}
