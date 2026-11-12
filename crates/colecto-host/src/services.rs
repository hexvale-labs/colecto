//! Owner-only local service controls and streaming Unix upstream transport.
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

#[derive(Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub address: String,
    pub token_file: PathBuf,
}
impl Upstream {
    fn validate(&self) -> Result<(), String> {
        let path = self
            .address
            .strip_prefix("unix:")
            .ok_or("Use unix:/absolute/socket/path")?;
        if !Path::new(path).is_absolute() || !self.token_file.is_absolute() {
            return Err("Socket and token paths must be absolute".into());
        }
        crate::runtime::read_token(&self.token_file).map_err(|e| e.to_string())?;
        Ok(())
    }
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
        })
    }
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
            let value: Option<Upstream> =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if let Some(value) = &value {
                value.validate()?;
            }
            let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
            let temporary = control.file.with_extension("tmp");
            std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
            std::fs::rename(temporary, &control.file).map_err(|e| e.to_string())?;
            *control.upstream.lock().unwrap() = value.clone();
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
        return proxy(upstream, request).await.unwrap_or_else(|e| {
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
#[cfg(unix)]
async fn proxy(upstream: Upstream, mut request: Request) -> Result<Response, String> {
    let token = zeroize::Zeroizing::new(
        crate::runtime::read_token(&upstream.token_file).map_err(|e| e.to_string())?,
    );
    let stream = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::net::UnixStream::connect(&upstream.address[5..]),
    )
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(hyper_util::rt::TokioIo::new(stream))
            .await
            .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    strip_hop_headers(request.headers_mut());
    for name in ["proxy-authorization", "cookie"] {
        request.headers_mut().remove(name);
    }
    request
        .headers_mut()
        .insert("host", "localhost".parse().unwrap());
    request
        .headers_mut()
        .insert("origin", "http://localhost".parse().unwrap());
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", token.as_str())
            .parse()
            .map_err(|_| "Invalid token")?,
    );
    let response = tokio::time::timeout(Duration::from_secs(30), sender.send_request(request))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let mut response = response.map(axum::body::Body::new);
    strip_hop_headers(response.headers_mut());
    Ok(response)
}
#[cfg(not(unix))]
async fn proxy(_: Upstream, _: Request) -> Result<Response, String> {
    Err("Unix sockets unsupported".into())
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
        "# Managed by Colecto\n[Unit]\nDescription=Colecto {name}\n{ordering}\n\n[Service]\nType=simple\nExecStart={exec}\n{environment}\nUMask=0077\nRestart=on-failure\nRestartSec=5\nNoNewPrivileges=true\n\n[Install]\nWantedBy={target}\n"
    ))
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
            let unit = format!("colecto-{name}.service");
            values.insert(name.into(), serde_json::json!({"installed":directory.join(&unit).exists(),"enabled":supported && control.run(&["is-enabled", &unit]).await.unwrap_or(false),"active":supported && control.run(&["is-active", &unit]).await.unwrap_or(false),"supported":supported,"reason":if supported {None} else {Some("Local systemd user manager or service executable unavailable")},"socket":socket_path().map(|p| p.to_string_lossy().into_owned()),"token_file":std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state/colecto/owner-token"))}));
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
            std::fs::write(&path, unit(&name, control.command.as_ref().unwrap())?)
                .map_err(|e| e.to_string())?;
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
        for address in ["https://example.com", "unix:relative"] {
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
