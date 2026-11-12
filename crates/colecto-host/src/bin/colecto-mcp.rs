//! Bounded stdio broker; the running Host owns all policy and approval state.
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use zeroize::Zeroizing;

const MAX_REQUEST_BYTES: usize = 512_000;
// 100 messages, 100000 body bytes each, worst-case JSON escaping twice
// (the MCP text content itself contains JSON), plus envelope and metadata space.
const MAX_RESPONSE_BYTES: usize =
    colecto_core::MAX_LIST_MESSAGES * colecto_core::MAX_BODY_BYTES * 12 + 10_000_000;

async fn run() -> Result<(), String> {
    let endpoint = std::env::args().nth(1).ok_or("Usage: colecto-mcp https://host/api/v1/mcp (credential via COLECTO_MCP_KEY_FILE or COLECTO_MCP_KEY)")?;
    let url = reqwest::Url::parse(&endpoint).map_err(|_| "Invalid MCP URL")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/api/v1/mcp"
        || !(url.scheme() == "https"
            || (url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))))
    {
        return Err(
            "Use an HTTPS MCP URL, or HTTP on loopback, without credentials or query".into(),
        );
    }
    let key = Zeroizing::new(if let Ok(path) = std::env::var("COLECTO_MCP_KEY_FILE") {
        let meta =
            std::fs::symlink_metadata(&path).map_err(|_| "Cannot read MCP credential file")?;
        if !meta.is_file() || meta.len() > 512 {
            return Err("Invalid MCP credential file".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err("MCP credential file must be private (0600)".into());
            }
        }
        std::fs::read_to_string(path).map_err(|_| "Cannot read MCP credential file")?
    } else {
        std::env::var("COLECTO_MCP_KEY")
            .map_err(|_| "Set COLECTO_MCP_KEY_FILE or COLECTO_MCP_KEY")?
    });
    let key = key.trim();
    if !key.starts_with("colecto_") || key.len() > 512 || key.contains(['\r', '\n']) {
        return Err("Use a scoped Colecto API key".into());
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|_| "Cannot initialize MCP transport")?;
    let mut input = tokio::io::BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    loop {
        let mut line = Vec::new();
        loop {
            let buffer = input
                .fill_buf()
                .await
                .map_err(|_| "Cannot read MCP input")?;
            if buffer.is_empty() {
                if line.is_empty() {
                    return Ok(());
                }
                return Err("Incomplete MCP input".into());
            }
            let len = buffer
                .iter()
                .position(|b| *b == b'\n')
                .map_or(buffer.len(), |i| i + 1);
            if line.len() + len > MAX_REQUEST_BYTES {
                return Err("MCP request exceeds 512000 bytes".into());
            }
            let done = buffer[len - 1] == b'\n';
            line.extend_from_slice(&buffer[..len]);
            input.consume(len);
            if done {
                break;
            }
        }
        let value: serde_json::Value =
            serde_json::from_slice(&line).map_err(|_| "Invalid MCP JSON")?;
        let mut request = client
            .post(url.clone())
            .bearer_auth(key)
            .header("Origin", url.origin().ascii_serialization())
            .header("Accept", "application/json, text/event-stream");
        if value["method"] != "initialize" {
            request = request.header("MCP-Protocol-Version", "2025-11-25");
        }
        let response = request
            .json(&value)
            .send()
            .await
            .map_err(|_| "Cannot reach MCP Host")?;
        if !response.status().is_success() {
            return Err(format!(
                "MCP Host returned HTTP {}",
                response.status().as_u16()
            ));
        }
        if response.status() == reqwest::StatusCode::ACCEPTED
            || response.status() == reqwest::StatusCode::NO_CONTENT
        {
            continue;
        }
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "MCP response interrupted")?
        {
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err("MCP response exceeds broker byte limit".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid MCP Host response")?;
        stdout
            .write_all(value.to_string().as_bytes())
            .await
            .map_err(|_| "Cannot write MCP output")?;
        stdout
            .write_all(b"\n")
            .await
            .map_err(|_| "Cannot write MCP output")?;
        stdout
            .flush()
            .await
            .map_err(|_| "Cannot flush MCP output")?;
    }
}
#[tokio::main]
async fn main() {
    colecto_host::logging::init();
    if let Err(error) = run().await {
        tracing::error!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn large_message_list_fits_broker() {
        let list = serde_json::json!(
            (0..100)
                .map(|_| serde_json::json!({"body":"\u{0001}".repeat(100_000)}))
                .collect::<Vec<_>>()
        );
        let response =
            serde_json::json!({"result":{"content":[{"type":"text","text":list.to_string()}]}});
        let bytes = serde_json::to_vec(&response).unwrap();
        assert!(bytes.len() > 1_000_000);
        assert!(bytes.len() <= super::MAX_RESPONSE_BYTES);
    }
}
