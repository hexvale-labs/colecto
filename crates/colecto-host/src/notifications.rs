//! Optional desktop session agent; the remote daemon remains the only mail synchronizer.
use std::io::{BufRead, BufReader};
use std::time::Duration;

pub fn run_agent(
    origin: &str,
    token_file: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = reqwest::Url::parse(origin)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Notification agent requires an HTTPS server origin".into());
    }
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut backoff = 1;
    loop {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let token = zeroize::Zeroizing::new(crate::runtime::read_token(token_file)?);
            let response = client
                .get(url.join("api/v1/events")?)
                .bearer_auth(token.as_str())
                .header("x-colecto-extension", "host")
                .header("x-colecto-account", "demo")
                .send()?
                .error_for_status()?;
            backoff = 1;
            consume_events(BufReader::new(response), || {
                let status = std::process::Command::new("notify-send")
                    .args([
                        "--app-name=Colecto",
                        "--",
                        "New mail",
                        "Open Colecto to read your messages.",
                    ])
                    .status()?;
                if !status.success() {
                    return Err(std::io::Error::other("notify-send failed"));
                }
                Ok(())
            })?;
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Notification connection unavailable: {error}");
        }
        std::thread::sleep(Duration::from_secs(backoff));
        backoff = (backoff * 2).min(60);
    }
}

fn consume_events(
    reader: impl BufRead,
    mut notify: impl FnMut() -> Result<(), std::io::Error>,
) -> Result<(), std::io::Error> {
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
                    .and_then(|v| v["ids"].as_array().map(|a| !a.is_empty()))
                    .unwrap_or(false)
            {
                notify()?;
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
        let input = b"event: ready\ndata: {}\n\nevent: mail\ndata: {\"ids\":[]}\n\nevent: mail\ndata: invalid\n\nevent: mail\ndata: {\"account\":\"a\",\"ids\":[\"new\"]}\n\n";
        let mut count = 0;
        consume_events(&input[..], || {
            count += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(count, 1);
    }
    #[test]
    fn oversized_events_are_bounded() {
        assert!(consume_events(&vec![b'x'; 65538][..], || Ok(())).is_err());
    }
}
