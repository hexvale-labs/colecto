//! Owner-only local service controls and the streaming upstream proxy: to a daemon's
//! Unix socket, or to a remote daemon's HTTPS origin.
use axum::{
    Json,
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

/// The daemon this Host forwards API requests to: `unix:/absolute/socket/path`, or an
/// `https://` origin (plain `http://` only on loopback). `token_file` holds its owner token.
#[derive(Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub address: String,
    pub token_file: PathBuf,
}
impl Upstream {
    fn validate(&self) -> Result<(), String> {
        if let Some(path) = self.address.strip_prefix("unix:") {
            if !Path::new(path).is_absolute() {
                return Err("Socket and token paths must be absolute".into());
            }
        } else {
            remote_origin(&self.address)?;
        }
        if !self.token_file.is_absolute() {
            return Err("Socket and token paths must be absolute".into());
        }
        crate::runtime::read_token(&self.token_file).map_err(|e| e.to_string())?;
        Ok(())
    }
}
/// `PUT /api/v1/upstream`: a socket with its token file, or a remote origin with its token.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamInput {
    address: String,
    #[serde(default)]
    token_file: Option<PathBuf>,
    #[serde(default)]
    token: Option<String>,
}
/// The remote daemon's token lives here, in the private data directory, never in `upstream.json`.
const REMOTE_TOKEN_FILE: &str = "upstream-token";

/// Normalize a remote daemon address to its origin. HTTPS is required, except for a daemon
/// on this computer (loopback), which may use plain HTTP.
fn remote_origin(address: &str) -> Result<String, String> {
    let invalid = || {
        "Use an https:// server address without a path, credentials or query (http:// only for localhost), or unix:/absolute/socket/path".to_string()
    };
    let url = reqwest::Url::parse(address).map_err(|_| invalid())?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(url.origin().ascii_serialization())
}
type Runner =
    fn(
        Vec<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, String>> + Send>>;
#[derive(Clone)]
pub(crate) struct Control {
    runner: Runner,
    local: bool,
    command: Option<Vec<String>>,
    file: PathBuf,
    upstream: Arc<Mutex<Option<Upstream>>>,
    /// Pooled connections to a remote daemon, so each request skips the TLS handshake.
    client: reqwest::Client,
}
impl Control {
    async fn run(&self, args: &[&str]) -> Result<bool, String> {
        (self.runner)(args.iter().map(|s| s.to_string()).collect()).await
    }
    pub fn open(
        directory: &Path,
        local: bool,
        command: Option<Vec<String>>,
    ) -> Result<Self, String> {
        let file = directory.join("upstream.json");
        let upstream: Option<Upstream> = if !local {
            None
        } else {
            match std::fs::read(&file) {
                Ok(bytes) => {
                    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid upstream: {e}"))?
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.to_string()),
            }
        };
        if let Some(value) = &upstream {
            value.validate()?;
        }
        Ok(Self {
            runner: |args| {
                Box::pin(async move {
                    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
                    systemctl(&refs).await
                })
            },
            local,
            command,
            file,
            upstream: Arc::new(Mutex::new(if local { upstream } else { None })),
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?,
        })
    }
    /// Turn owner input into a stored upstream. A remote token is checked against the daemon
    /// first, then written to a private file; it is never returned or logged.
    async fn prepare(&self, input: UpstreamInput) -> Result<Upstream, String> {
        if input.address.starts_with("unix:") {
            if input.token.is_some() {
                return Err("A socket uses its daemon's token file, not a pasted token".into());
            }
            return Ok(Upstream {
                address: input.address,
                token_file: input.token_file.ok_or("Choose the daemon's token file")?,
            });
        }
        let origin = remote_origin(&input.address)?;
        let token_file = self
            .file
            .parent()
            .ok_or("Invalid data directory")?
            .join(REMOTE_TOKEN_FILE);
        let Some(token) = input.token.map(zeroize::Zeroizing::new) else {
            // Re-saving the current server keeps its stored token.
            let current = self.upstream.lock().unwrap().clone();
            return match current {
                Some(current) if current.address == origin => Ok(current),
                _ => Err("Enter the server's access token".into()),
            };
        };
        let token = token.trim();
        if !crate::service::valid_token(token) {
            return Err(
                "Access token must contain 64–512 ASCII letters, digits, hyphens or underscores"
                    .into(),
            );
        }
        let response = tokio::time::timeout(
            Duration::from_secs(15),
            self.client
                .post(format!("{origin}/api/v1/session"))
                .header("origin", &origin)
                .bearer_auth(token)
                .send(),
        )
        .await
        .map_err(|_| "The server did not answer in time".to_string())?
        .map_err(|_| unreachable_server(&origin))?;
        if !response.status().is_success() {
            return Err("The server did not accept this access token".into());
        }
        write_private(&token_file, token.as_bytes())?;
        Ok(Upstream {
            address: origin,
            token_file,
        })
    }
}
fn unreachable_server(origin: &str) -> String {
    format!("Cannot reach the mail server at {origin}. Check the address and your connection.")
}
/// Write a file readable only by its owner, replacing any previous one atomically.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let temporary = path.with_extension("tmp");
    let _ = std::fs::remove_file(&temporary);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
}
fn failure(message: impl ToString) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error":message.to_string()})),
    )
        .into_response()
}
pub(crate) async fn handle(
    State(control): State<Control>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    if !path.starts_with("/api/v1/") {
        return next.run(request).await;
    }
    let owner = matches!(
        request.extensions().get::<crate::api_keys::Principal>(),
        Some(crate::api_keys::Principal::Owner)
    );
    // Installing units and choosing the upstream are owner actions, whatever reaches this layer.
    if (path == "/api/v1/upstream" || path.starts_with("/api/v1/services")) && !owner {
        return StatusCode::FORBIDDEN.into_response();
    }
    if path == "/api/v1/upstream" {
        if !control.local {
            return StatusCode::FORBIDDEN.into_response();
        }
        if request.method() == Method::GET {
            // Wrapped so "none" is an object, not a bare JSON null.
            return Json(serde_json::json!({"upstream": control.upstream.lock().unwrap().clone()}))
                .into_response();
        }
        if request.method() != Method::PUT {
            return StatusCode::METHOD_NOT_ALLOWED.into_response();
        }
        let result = async {
            let bytes = axum::body::to_bytes(request.into_body(), 8192)
                .await
                .map_err(|e| e.to_string())?;
            let input: Option<UpstreamInput> =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let value = match input {
                Some(input) => Some(control.prepare(input).await?),
                None => None,
            };
            if let Some(value) = &value {
                value.validate()?;
            }
            let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
            let temporary = control.file.with_extension("tmp");
            std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
            std::fs::rename(temporary, &control.file).map_err(|e| e.to_string())?;
            let previous = std::mem::replace(&mut *control.upstream.lock().unwrap(), value.clone());
            // A remote token that is no longer used does not stay on disk.
            if let Some(previous) = previous
                && previous.token_file.file_name() == Some(REMOTE_TOKEN_FILE.as_ref())
                && value
                    .as_ref()
                    .is_none_or(|v| v.token_file != previous.token_file)
            {
                let _ = std::fs::remove_file(&previous.token_file);
            }
            Ok::<_, String>(Json(serde_json::json!({"upstream": value})).into_response())
        }
        .await;
        return result.unwrap_or_else(failure);
    }
    if path == "/api/v1/services" || path.starts_with("/api/v1/services/") {
        if !control.local {
            return StatusCode::FORBIDDEN.into_response();
        }
        return service_request(&control, request)
            .await
            .unwrap_or_else(failure);
    }
    let upstream = control.upstream.lock().unwrap().clone();
    if let Some(upstream) = upstream {
        // Scoped API keys must never be promoted to the daemon owner.
        if !owner {
            return StatusCode::FORBIDDEN.into_response();
        }
        return proxy(&control.client, upstream, request)
            .await
            .unwrap_or_else(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({"error":e})),
                )
                    .into_response()
            });
    }
    next.run(request).await
}
fn strip_hop_headers(headers: &mut axum::http::HeaderMap) {
    let nominated = headers
        .get("connection")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_owned())
        .collect::<Vec<_>>();
    for name in nominated {
        headers.remove(name);
    }
    for name in [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        headers.remove(name);
    }
}
/// Forward one owner request to the upstream daemon with its owner bearer, streaming the
/// response (including SSE) back. Cookies and proxy credentials never cross.
async fn proxy(
    client: &reqwest::Client,
    upstream: Upstream,
    mut request: Request,
) -> Result<Response, String> {
    let token = zeroize::Zeroizing::new(
        crate::runtime::read_token(&upstream.token_file).map_err(|e| e.to_string())?,
    );
    strip_hop_headers(request.headers_mut());
    for name in ["proxy-authorization", "cookie", "host", "origin"] {
        request.headers_mut().remove(name);
    }
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", token.as_str())
            .parse()
            .map_err(|_| "Invalid token")?,
    );
    let mut response = match upstream.address.strip_prefix("unix:") {
        Some(path) => proxy_socket(Path::new(path), request).await?,
        None => proxy_remote(client, &upstream.address, request).await?,
    };
    strip_hop_headers(response.headers_mut());
    Ok(response)
}
/// Largest request body forwarded to a remote daemon (sending allows 25 MB of attachments).
const MAX_REMOTE_BODY: usize = 40_000_000;
async fn proxy_remote(
    client: &reqwest::Client,
    origin: &str,
    request: Request,
) -> Result<Response, String> {
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, MAX_REMOTE_BODY)
        .await
        .map_err(|_| "Request too large".to_string())?;
    let path = parts.uri.path_and_query().map_or("/", |path| path.as_str());
    let mut headers = parts.headers;
    headers.remove("content-length");
    // The daemon checks Origin against its public origin; Host follows the URL.
    headers.insert("origin", origin.parse().map_err(|_| "Invalid origin")?);
    let sent = client
        .request(parts.method, format!("{origin}{path}"))
        .headers(headers)
        .body(body)
        .send();
    let response = tokio::time::timeout(Duration::from_secs(30), sent)
        .await
        .map_err(|_| "The mail server did not answer in time.".to_string())?
        .map_err(|_| unreachable_server(origin))?;
    let mut builder = Response::builder().status(response.status());
    for (name, value) in response.headers() {
        if name != "content-length" {
            builder = builder.header(name, value);
        }
    }
    // Chunks pass through as they arrive, so event streams stay live.
    let stream = futures_util::stream::unfold(Some(response), |response| async move {
        let mut response = response?;
        match response.chunk().await {
            Ok(Some(chunk)) => Some((Ok(chunk), Some(response))),
            Ok(None) => None,
            Err(error) => Some((Err(std::io::Error::other(error)), None)),
        }
    });
    builder
        .body(axum::body::Body::from_stream(stream))
        .map_err(|e| e.to_string())
}
#[cfg(unix)]
async fn proxy_socket(path: &Path, mut request: Request) -> Result<Response, String> {
    let stream = connect_upstream(path, RESTART_GRACE).await?;
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(hyper_util::rt::TokioIo::new(stream))
            .await
            .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    request
        .headers_mut()
        .insert("host", "localhost".parse().unwrap());
    request
        .headers_mut()
        .insert("origin", "http://localhost".parse().unwrap());
    let response = tokio::time::timeout(Duration::from_secs(30), sender.send_request(request))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(response.map(axum::body::Body::new))
}
#[cfg(not(unix))]
async fn proxy_socket(_: &Path, _: Request) -> Result<Response, String> {
    Err("Unix sockets unsupported".into())
}
/// How long a request waits for the daemon's socket. Restarting the systemd unit (as
/// "Update" does) removes its runtime directory, so the socket is briefly missing or refuses
/// connections; requests in that window wait instead of failing with ENOENT.
const RESTART_GRACE: Duration = Duration::from_secs(10);
#[cfg(unix)]
async fn connect_upstream(path: &Path, grace: Duration) -> Result<tokio::net::UnixStream, String> {
    let deadline = tokio::time::Instant::now() + grace;
    loop {
        match tokio::time::timeout_at(deadline, tokio::net::UnixStream::connect(path)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Ok(Err(error)) => {
                return Err(format!(
                    "The background mail service is not running ({error}). Start it in Settings → Connection."
                ));
            }
            Err(_) => return Err("The background mail service did not answer in time.".into()),
        }
    }
}

fn config_home() -> Result<PathBuf, String> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok_or("HOME unavailable".into())
}
fn socket_path() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|p| PathBuf::from(p).join("colecto/mail.sock"))
}
fn quote(value: &str) -> Result<String, String> {
    if value.chars().any(|c| c.is_control()) {
        return Err("Executable path contains control characters".into());
    }
    Ok(format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    ))
}
fn unit(name: &str, command: &[String]) -> Result<String, String> {
    let mut args = command.to_vec();
    if name == "notifications" {
        if args.last().is_some_and(|a| a == "daemon") {
            args.pop();
        }
        args.push("notify".into());
    }
    let exec = args
        .iter()
        .map(|s| quote(s))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    let environment = if name == "daemon" {
        "Environment=COLECTO_SOCKET=%t/colecto/mail.sock\nRuntimeDirectory=colecto\nRuntimeDirectoryMode=0700\nEnvironment=COLECTO_DATA_DIR=%h/.local/state/colecto\nEnvironmentFile=-%h/.config/colecto/daemon.env"
    } else {
        "EnvironmentFile=-%h/.config/colecto/notifications.env\nEnvironment=COLECTO_PUBLIC_ORIGIN=unix:%t/colecto/mail.sock\nEnvironment=COLECTO_TOKEN_FILE=%h/.local/state/colecto/owner-token"
    };
    let (ordering, target) = if name == "daemon" {
        (
            "After=network-online.target\nWants=network-online.target",
            "default.target",
        )
    } else {
        (
            "PartOf=graphical-session.target\nAfter=graphical-session.target",
            "graphical-session.target",
        )
    };
    Ok(format!(
        "# Managed by Colecto\n{VERSION_MARKER}{version}\n[Unit]\nDescription=Colecto {name}\n{ordering}\n\n[Service]\nType=simple\nExecStart={exec}\n{environment}\nUMask=0077\nRestart=on-failure\nRestartSec=5\nNoNewPrivileges=true\n\n[Install]\nWantedBy={target}\n",
        version = crate::VERSION
    ))
}
const VERSION_MARKER: &str = "# Colecto-Version: ";
fn release(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split(['.', '-', '+']).map(|p| p.parse().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}
/// The Colecto version an installed unit should be updated to, if any. Units from before
/// versioning have no marker and count as outdated; a unit written by a newer Colecto is
/// left alone so opening an older app never downgrades it.
fn update_to(installed: &str, expected: &str) -> Option<&'static str> {
    if installed == expected {
        return None;
    }
    let version = installed
        .lines()
        .find_map(|l| l.strip_prefix(VERSION_MARKER))
        .and_then(release);
    match (version, release(crate::VERSION)) {
        (Some(version), Some(current)) if version > current => None,
        _ => Some(crate::VERSION),
    }
}
fn write_unit(path: &Path, text: &str) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, text).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn managed(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err("Refusing a symlink unit".into()),
        Ok(_) => {
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            if !text.lines().any(|l| l == "# Managed by Colecto") {
                return Err(
                    "This unit was not written by Colecto; manage it outside the app".into(),
                );
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
async fn systemctl(args: &[&str]) -> Result<bool, String> {
    let mut command = tokio::process::Command::new("systemctl");
    command
        .arg("--user")
        .args(args)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    tokio::time::timeout(Duration::from_secs(10), command.status())
        .await
        .map_err(|_| "systemctl timed out".to_string())?
        .map(|s| s.success())
        .map_err(|e| e.to_string())
}
async fn service_request(control: &Control, request: Request) -> Result<Response, String> {
    if !cfg!(target_os = "linux") {
        if request.method() != Method::GET {
            return Err("User services are supported only on Linux".into());
        }
        let status = serde_json::json!({"installed":false,"enabled":false,"active":false,"supported":false,"reason":"User services are supported only on Linux"});
        return Ok(
            Json(serde_json::json!({"daemon":status,"notifications":status})).into_response(),
        );
    }
    let supported = cfg!(target_os = "linux")
        && control.command.is_some()
        && control.run(&["show-environment"]).await.unwrap_or(false);
    let directory = config_home()?.join("systemd/user");
    if request.method() == Method::GET && request.uri().path() == "/api/v1/services" {
        let mut values = serde_json::Map::new();
        for name in ["daemon", "notifications"] {
            let unit_name = format!("colecto-{name}.service");
            let text = std::fs::read_to_string(directory.join(&unit_name)).ok();
            let version = text.as_deref().and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix(VERSION_MARKER))
                    .map(str::to_owned)
            });
            let expected = control.command.as_ref().map(|c| unit(name, c));
            let update = match (&text, expected) {
                (Some(text), Some(Ok(expected))) if supported => update_to(text, &expected),
                _ => None,
            };
            values.insert(name.into(), serde_json::json!({"installed":text.is_some(),"version":version,"update_to":update,"enabled":supported && control.run(&["is-enabled", &unit_name]).await.unwrap_or(false),"active":supported && control.run(&["is-active", &unit_name]).await.unwrap_or(false),"supported":supported,"reason":if supported {None} else {Some("Local systemd user manager or service executable unavailable")},"socket":socket_path().map(|p| p.to_string_lossy().into_owned()),"token_file":std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state/colecto/owner-token"))}));
        }
        return Ok(Json(values).into_response());
    }
    if !supported {
        return Err("User services unavailable".into());
    }
    if request.method() != Method::POST {
        return Ok(StatusCode::METHOD_NOT_ALLOWED.into_response());
    }
    let name = request
        .uri()
        .path()
        .strip_prefix("/api/v1/services/")
        .ok_or("Unknown service")?
        .to_owned();
    if !["daemon", "notifications"].contains(&name.as_str()) {
        return Err("Unknown service".into());
    }
    let bytes = axum::body::to_bytes(request.into_body(), 4096)
        .await
        .map_err(|e| e.to_string())?;
    let input: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let filename = format!("colecto-{name}.service");
    let path = directory.join(&filename);
    managed(&path)?;
    let action = input["action"].as_str().ok_or("Missing action")?;
    if action != "install" && !path.exists() {
        return Err("Service is not installed by Colecto".into());
    }
    let args = match action {
        "install" => {
            let autostart = input["autostart"].as_bool().ok_or("Missing autostart")?;
            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            write_unit(&path, &unit(&name, control.command.as_ref().unwrap())?)?;
            if !control.run(&["daemon-reload"]).await? {
                return Err("systemctl daemon-reload failed".into());
            }
            if autostart {
                vec!["enable", "--now", &filename]
            } else {
                vec!["start", &filename]
            }
        }
        "autostart" => vec![
            if input["enabled"].as_bool().ok_or("Missing enabled")? {
                "enable"
            } else {
                "disable"
            },
            &filename,
        ],
        // Rewrites the unit for this Colecto and its executable. Only a running service is
        // restarted, so it picks up the new binary; reenable moves links if WantedBy changed.
        "update" => {
            let enabled = control.run(&["is-enabled", &filename]).await?;
            write_unit(&path, &unit(&name, control.command.as_ref().unwrap())?)?;
            if !control.run(&["daemon-reload"]).await? {
                return Err("systemctl daemon-reload failed".into());
            }
            if enabled && !control.run(&["reenable", &filename]).await? {
                return Err("systemctl reenable failed".into());
            }
            vec!["try-restart", &filename]
        }
        "start" => vec!["start", &filename],
        "stop" => vec!["stop", &filename],
        "uninstall" => vec!["disable", "--now", &filename],
        _ => return Err("Unknown action".into()),
    };
    if !control.run(&args).await? {
        return Err("systemctl operation failed".into());
    }
    if action == "uninstall" {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
        if !control.run(&["daemon-reload"]).await? {
            return Err("systemctl daemon-reload failed".into());
        }
    }
    Ok(Json(true).into_response())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_units() {
        let command = vec!["/opt/My App/colecto".into(), "daemon".into()];
        let daemon = unit("daemon", &command).unwrap();
        assert!(daemon.contains("ExecStart=\"/opt/My App/colecto\" \"daemon\""));
        assert!(daemon.contains("RuntimeDirectoryMode=0700"));
        let notify = unit("notifications", &command).unwrap();
        assert!(notify.contains("\"notify\""));
        assert!(!notify.contains("\"daemon\""));
        assert!(notify.contains("unix:%t/colecto/mail.sock"));
        assert!(notify.contains("WantedBy=graphical-session.target"));
        assert!(daemon.contains(&format!("\n# Colecto-Version: {}\n", crate::VERSION)));
    }
    #[test]
    fn unit_updates() {
        let current = unit("daemon", &["/usr/bin/colecto".into(), "daemon".into()]).unwrap();
        assert_eq!(update_to(&current, &current), None);
        // Units from before versioning, older versions and moved executables are offered.
        let legacy = current.replace(&format!("# Colecto-Version: {}\n", crate::VERSION), "");
        assert_eq!(update_to(&legacy, &current), Some(crate::VERSION));
        let old = current.replace(crate::VERSION, "0.0.1");
        assert_eq!(update_to(&old, &current), Some(crate::VERSION));
        let moved = current.replace("/usr/bin", "/opt");
        assert_eq!(update_to(&moved, &current), Some(crate::VERSION));
        // An older app never downgrades a unit written by a newer one.
        let newer = current.replace(crate::VERSION, "999.0.0");
        assert_eq!(update_to(&newer, &current), None);
    }
    #[test]
    fn foreign_units_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("unit");
        std::fs::write(&path, "[Unit]").unwrap();
        assert!(managed(&path).is_err());
        std::fs::write(&path, "# Managed by Colecto\n[Unit]").unwrap();
        assert!(managed(&path).is_ok());
    }
    #[test]
    fn invalid_upstream() {
        for address in [
            "https://example.com",
            "unix:relative",
            "http://example.com",
            "https://example.com/path",
            "https://user@example.com",
        ] {
            assert!(
                Upstream {
                    address: address.into(),
                    token_file: "/missing".into()
                }
                .validate()
                .is_err()
            );
        }
    }
    #[tokio::test]
    async fn remote_controls_refused_without_running_commands() {
        let directory = tempfile::tempdir().unwrap();
        let mut control = Control::open(directory.path(), false, None).unwrap();
        control.runner = |_| panic!("must not run systemctl");
        let router = axum::Router::new()
            .fallback(|| async { StatusCode::NOT_FOUND })
            .layer(axum::middleware::from_fn_with_state(control, handle));
        use tower::ServiceExt;
        for path in ["/api/v1/services", "/api/v1/upstream"] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }
    /// Regression: updating the daemon unit restarts it, and systemd removes the socket's
    /// runtime directory meanwhile. Requests in that window must wait, not fail with ENOENT.
    #[cfg(unix)]
    #[tokio::test]
    async fn upstream_connect_waits_for_a_restarting_daemon() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("colecto/mail.sock");
        let path = socket.clone();
        let restart = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let listener = tokio::net::UnixListener::bind(&path).unwrap();
            listener.accept().await.unwrap();
        });
        connect_upstream(&socket, Duration::from_secs(5))
            .await
            .expect("waits for the socket to come back");
        restart.await.unwrap();
        // A daemon that stays down still fails, with a message that says what to do.
        let missing = dir.path().join("gone.sock");
        let error = connect_upstream(&missing, Duration::from_millis(300))
            .await
            .unwrap_err();
        assert!(error.contains("not running"), "{error}");
    }
    /// A remote daemon over loopback HTTP takes the same pooled-client path as HTTPS.
    #[tokio::test]
    async fn remote_upstream_checks_token_stores_it_privately_and_streams() {
        use tower::ServiceExt;
        let dir = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let authority = listener.local_addr().unwrap().to_string();
        let data = dir.path().join("daemon");
        let daemon = crate::MailService::new(
            crate::runtime::open_accounts(&data).unwrap(),
            crate::AccessPolicy::local(authority.clone()),
            dir.path().join("assets"),
        )
        .unwrap();
        let daemon_token = crate::runtime::read_token(&data.join("owner-token")).unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, daemon.router()).await.unwrap();
        });
        let local_data = dir.path().join("local");
        let local = crate::MailService::new(
            crate::runtime::open_accounts(&local_data).unwrap(),
            crate::AccessPolicy::local("localhost".into()),
            dir.path().join("assets"),
        )
        .unwrap();
        let token = crate::runtime::local_owner_token(&local_data).unwrap();
        let request = |path: &str, method: &str, body: String| {
            Request::builder()
                .uri(path)
                .method(method)
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .header("x-colecto-extension", "host")
                .header("x-colecto-account", "demo")
                .body(axum::body::Body::from(body))
                .unwrap()
        };
        let origin = format!("http://{authority}");
        let put = |token: &str| {
            request(
                "/api/v1/upstream",
                "PUT",
                serde_json::json!({"address": origin, "token": token}).to_string(),
            )
        };
        let wrong = "x".repeat(64);
        let response = local.router().oneshot(put(&wrong)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let token_file = local_data.join(REMOTE_TOKEN_FILE);
        assert!(!token_file.exists());
        let response = local.router().oneshot(put(&daemon_token)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 10_000)
            .await
            .unwrap();
        let body = String::from_utf8_lossy(&body);
        assert!(!body.contains(&daemon_token), "the token is never returned");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&token_file).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0);
        }
        let response = local
            .router()
            .oneshot(request("/api/v1/sync", "GET", String::new()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = local
            .router()
            .oneshot(request("/api/v1/events", "GET", String::new()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        use futures_util::StreamExt;
        let mut stream = response.into_body().into_data_stream();
        let chunk = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(String::from_utf8_lossy(&chunk).contains("ready"));
        let response = local
            .router()
            .oneshot(request(
                "/api/v1/preferences",
                "POST",
                r#"{"mcp":true}"#.into(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = local
            .router()
            .oneshot(request("/api/v1/upstream", "PUT", "null".into()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!token_file.exists(), "an unused remote token is removed");
        server.abort();
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn proxy_roundtrip_streams_to_real_socket_host() {
        use tower::ServiceExt;
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("daemon");
        let accounts = crate::runtime::open_accounts(&data).unwrap();
        let daemon = crate::MailService::new(
            accounts,
            crate::AccessPolicy::local("localhost".into()),
            dir.path().join("assets"),
        )
        .unwrap();
        let socket = dir.path().join("mail.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, daemon.router()).await.unwrap();
        });
        let local_data = dir.path().join("local");
        let local = crate::MailService::new(
            crate::runtime::open_accounts(&local_data).unwrap(),
            crate::AccessPolicy::local("localhost".into()),
            dir.path().join("assets"),
        )
        .unwrap();
        let token = crate::runtime::local_owner_token(&local_data).unwrap();
        let upstream = Upstream {
            address: format!("unix:{}", socket.display()),
            token_file: data.join("owner-token"),
        };
        let request = |path: &str, method: &str, body: String| {
            Request::builder()
                .uri(path)
                .method(method)
                .header("host", "localhost")
                .header("origin", "http://localhost")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .header("x-colecto-extension", "host")
                .header("x-colecto-account", "demo")
                .body(axum::body::Body::from(body))
                .unwrap()
        };
        let response = local
            .router()
            .oneshot(request(
                "/api/v1/upstream",
                "PUT",
                serde_json::to_string(&upstream).unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = local
            .router()
            .oneshot(request("/api/v1/accounts", "GET", String::new()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("demo"));
        let response = local
            .router()
            .oneshot(request("/api/v1/events", "GET", String::new()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        use futures_util::StreamExt;
        let mut stream = response.into_body().into_data_stream();
        let chunk = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(String::from_utf8_lossy(&chunk).contains("ready"));
        let response = local
            .router()
            .oneshot(request(
                "/api/v1/preferences",
                "POST",
                r#"{"mcp":true}"#.into(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = local
            .router()
            .oneshot(request("/api/v1/upstream", "PUT", "null".into()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        server.abort();
    }
}
