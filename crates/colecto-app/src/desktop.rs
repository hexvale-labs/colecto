//! The desktop app without a listening port: the window loads Colecto from the `colecto://`
//! scheme, which hands each request to the in-process mail service, and live events arrive
//! over a Tauri channel. A loopback port opens only while API keys (MCP) are turned on, and
//! it answers API keys only (docs/architecture.md, "Desktop app").
use axum::{
    Router,
    body::Body,
    http::{HeaderValue, Request, Response, StatusCode, header},
};
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    net::Ipv4Addr,
    sync::{Arc, Mutex, OnceLock},
};
use tower::ServiceExt;

/// The custom scheme the window loads; registered schemes count as the app's own pages, so
/// only they reach the app's commands.
pub const SCHEME: &str = "colecto";
/// What the mail service sees as its authority and origin for every in-window request.
pub const AUTHORITY: &str = "colecto.localhost";
const ORIGIN: &str = "http://colecto.localhost";
/// The page's own origin: WebView2 and Android map custom schemes onto `http://<scheme>.localhost`.
#[cfg(windows)]
pub const PAGE_ORIGIN: &str = "http://colecto.localhost";
#[cfg(not(windows))]
pub const PAGE_ORIGIN: &str = "colecto://localhost";
/// Responses larger than this are refused rather than buffered (the largest attachment the
/// service serves is well below it).
const MAX_RESPONSE: usize = 512 * 1024 * 1024;
/// Routes that stream for as long as the window lives; they go through [`stream`] instead.
const STREAMS: &[&str] = &["/api/v1/events", "/api/v1/extensions/events"];

/// The mail service's router, set once the app's setup has built it.
pub type Shared = Arc<OnceLock<Router>>;

/// Turn a request from the window into one the mail service accepts: its authority, and its
/// origin only when the request really came from Colecto's own page.
fn inward(request: axum::http::Request<Vec<u8>>) -> Request<Body> {
    let (mut parts, body) = request.into_parts();
    let path = parts
        .uri
        .path_and_query()
        .map_or("/", |p| p.as_str())
        .to_owned();
    parts.uri = path.parse().unwrap_or_default();
    // WebKitGTK sends no `Origin` on the page's own same-origin requests to a custom scheme.
    // Only this window can reach the scheme at all, so a missing origin is the page itself;
    // any other origin (a sandboxed frame's `null`, a web page) stays foreign and is refused.
    let origin = match parts.headers.get(header::ORIGIN) {
        Some(origin) if origin == PAGE_ORIGIN => ORIGIN,
        Some(_) => "null",
        None => ORIGIN,
    };
    parts
        .headers
        .insert(header::ORIGIN, HeaderValue::from_static(origin));
    parts
        .headers
        .insert(header::HOST, HeaderValue::from_static(AUTHORITY));
    Request::from_parts(parts, Body::from(body))
}

/// Answer one `colecto://` request from the in-process service.
pub async fn serve(
    router: &Shared,
    request: axum::http::Request<Vec<u8>>,
) -> axum::http::Response<Vec<u8>> {
    let streaming = STREAMS.contains(&request.uri().path());
    let Some(router) = router.get().cloned() else {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    };
    if streaming {
        return status(StatusCode::NOT_FOUND);
    }
    let Ok(response) = router.oneshot(inward(request)).await;
    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, MAX_RESPONSE).await else {
        return status(StatusCode::BAD_GATEWAY);
    };
    allow_ipc(&mut parts.headers);
    axum::http::Response::from_parts(parts, bytes.to_vec())
}
fn status(code: StatusCode) -> axum::http::Response<Vec<u8>> {
    let mut response = axum::http::Response::new(Vec::new());
    *response.status_mut() = code;
    response
}
/// Tauri's IPC goes through `ipc://localhost` (`http://ipc.localhost` on Windows); the mail
/// service's policy allows only `'self'`, which would push every call to a slower fallback.
fn allow_ipc(headers: &mut axum::http::HeaderMap) {
    if let Some(policy) = headers
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        && let Ok(value) = HeaderValue::from_str(&policy.replacen(
            "default-src 'self';",
            "default-src 'self'; connect-src 'self' ipc: http://ipc.localhost;",
            1,
        ))
    {
        headers.insert(header::CONTENT_SECURITY_POLICY, value);
    }
}

#[derive(Default)]
struct StreamsInner {
    generation: u64,
    next_token: u64,
    open: HashMap<u32, (u64, tokio::task::AbortHandle)>,
}

/// Open event streams by id, so the page can close one and a reload closes them all.
#[derive(Default)]
pub struct Streams(Mutex<StreamsInner>);
impl Streams {
    fn generation(&self) -> u64 {
        self.0.lock().map(|inner| inner.generation).unwrap_or(0)
    }

    fn register(&self, id: u32, generation: u64, handle: tokio::task::AbortHandle) -> Option<u64> {
        let Ok(mut inner) = self.0.lock() else {
            handle.abort();
            return None;
        };
        if inner.generation != generation {
            handle.abort();
            return None;
        }
        inner.next_token = inner.next_token.wrapping_add(1);
        let token = inner.next_token;
        if let Some((_, old)) = inner.open.insert(id, (token, handle)) {
            old.abort();
        }
        Some(token)
    }

    fn remove_if_token(&self, id: u32, token: u64) {
        if let Ok(mut inner) = self.0.lock()
            && inner
                .open
                .get(&id)
                .is_some_and(|(current, _)| *current == token)
        {
            inner.open.remove(&id);
        }
    }

    pub fn close(&self, id: u32) {
        if let Some((_, stream)) = self.0.lock().ok().and_then(|mut s| s.open.remove(&id)) {
            stream.abort();
        }
    }

    pub fn close_all(&self) {
        if let Ok(mut inner) = self.0.lock() {
            inner.generation = inner.generation.wrapping_add(1);
            for (_, (_, stream)) in inner.open.drain() {
                stream.abort();
            }
        }
    }
}

/// Decode complete UTF-8 from `pending` + `chunk`, keeping any trailing incomplete multibyte
/// sequence in `pending` until the next chunk arrives.
fn decode_utf8_chunk(pending: &mut Vec<u8>, chunk: &[u8]) -> String {
    pending.extend_from_slice(chunk);
    let mut out = String::new();
    loop {
        match std::str::from_utf8(pending) {
            Ok(valid) => {
                out.push_str(valid);
                pending.clear();
                break;
            }
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to > 0 {
                    out.push_str(
                        std::str::from_utf8(&pending[..valid_up_to]).expect("valid UTF-8 prefix"),
                    );
                }
                match error.error_len() {
                    Some(invalid_len) => {
                        out.push('\u{FFFD}');
                        pending.drain(..valid_up_to + invalid_len);
                    }
                    None => {
                        pending.drain(..valid_up_to);
                        break;
                    }
                }
            }
        }
    }
    out
}

/// Flush any trailing incomplete UTF-8 bytes at EOF as U+FFFD replacement characters.
fn flush_utf8(pending: &mut Vec<u8>) -> String {
    if pending.is_empty() {
        String::new()
    } else {
        let text = String::from_utf8_lossy(pending).into_owned();
        pending.clear();
        text
    }
}

#[derive(serde::Deserialize)]
pub struct StreamRequest {
    path: String,
    headers: HashMap<String, String>,
    body: Option<String>,
}

/// Follow one of the service's event streams and pass its text on as it arrives. Ends when
/// the stream does; the page reconnects as it would over HTTP.
#[tauri::command]
pub async fn stream(
    request: StreamRequest,
    id: u32,
    channel: tauri::ipc::Channel<String>,
    router: tauri::State<'_, Shared>,
    streams: tauri::State<'_, Arc<Streams>>,
) -> Result<(), String> {
    if !STREAMS.contains(&request.path.as_str()) {
        return Err("Not an event stream".into());
    }
    let generation = streams.generation();
    let router = router.get().cloned().ok_or("Mail service is starting")?;
    let mut builder = axum::http::Request::builder()
        .method(if request.body.is_some() {
            "POST"
        } else {
            "GET"
        })
        .uri(&request.path);
    for (name, value) in &request.headers {
        // The page cannot choose its origin or authority; `inward` sets them.
        if !name.eq_ignore_ascii_case("origin") && !name.eq_ignore_ascii_case("host") {
            builder = builder.header(name, value);
        }
    }
    let request = builder
        .header(header::ORIGIN, PAGE_ORIGIN)
        .body(request.body.unwrap_or_default().into_bytes())
        .map_err(|_| "Invalid event request")?;
    let Ok(response) = router.oneshot(inward(request)).await;
    if !response.status().is_success() {
        return Err(format!("Event stream refused: {}", response.status()));
    }
    let mut body = response.into_body().into_data_stream();
    let task = tokio::spawn(async move {
        let mut pending = Vec::new();
        while let Some(Ok(chunk)) = body.next().await {
            let text = decode_utf8_chunk(&mut pending, &chunk);
            if !text.is_empty() && channel.send(text).is_err() {
                return;
            }
        }
        let tail = flush_utf8(&mut pending);
        if !tail.is_empty() {
            let _ = channel.send(tail);
        }
    });
    let Some(token) = streams.register(id, generation, task.abort_handle()) else {
        return Ok(());
    };
    let _ = task.await;
    streams.remove_if_token(id, token);
    Ok(())
}

#[tauri::command]
pub fn close_stream(id: u32, streams: tauri::State<'_, Arc<Streams>>) {
    streams.close(id);
}

/// The loopback port for API keys and MCP clients: open while MCP is on, closed otherwise.
/// It forwards only requests that carry an API key, so the owner's token never crosses it.
pub struct ApiPort {
    port: u16,
    router: Router,
    running: Mutex<Option<tokio::task::AbortHandle>>,
}
impl ApiPort {
    pub fn new(port: u16, router: Router) -> Self {
        Self {
            port,
            router,
            running: Mutex::new(None),
        }
    }
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
    /// Open or close the port; does nothing when it is already in that state.
    pub async fn set(&self, open: bool) {
        let Ok(running) = self.running.lock().map(|r| r.is_some()) else {
            return;
        };
        if open == running {
            return;
        }
        if !open {
            if let Some(task) = self.running.lock().ok().and_then(|mut r| r.take()) {
                task.abort();
                tracing::info!("API port closed");
            }
            return;
        }
        let listener = match tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, self.port)).await {
            Ok(listener) => listener,
            Err(error) => {
                tracing::warn!(port = self.port, %error, "Could not open the API port for MCP clients");
                return;
            }
        };
        let (port, router) = (self.port, self.router.clone());
        let app = Router::new().fallback(move |request: Request<Body>| {
            let router = router.clone();
            async move { from_loopback(port, router, request).await }
        });
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, app).await {
                tracing::warn!(%error, "API port stopped");
            }
        });
        if let Ok(mut slot) = self.running.lock() {
            *slot = Some(task.abort_handle());
        }
        tracing::info!(url = %self.origin(), "API port open for MCP clients");
    }
}

/// One request on the API port. Only loopback authorities (no DNS rebinding), only API keys,
/// and the browser origin of the port itself is translated to the service's.
async fn from_loopback(port: u16, router: Router, mut request: Request<Body>) -> Response<Body> {
    let refused = || {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::FORBIDDEN;
        response
    };
    let authorities = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    if !host.is_some_and(|h| authorities.iter().any(|a| a == h)) {
        return refused();
    }
    let key = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("Bearer colecto_"));
    // `/api/v1/version` tells clients what they reached and needs no credential.
    if !key && request.uri().path() != "/api/v1/version" {
        return refused();
    }
    let headers = request.headers_mut();
    match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        Some(origin) if authorities.iter().any(|a| origin == format!("http://{a}")) => {
            headers.insert(header::ORIGIN, HeaderValue::from_static(ORIGIN));
        }
        Some(_) => {
            headers.insert(header::ORIGIN, HeaderValue::from_static("null"));
        }
        None => {}
    }
    headers.insert(header::HOST, HeaderValue::from_static(AUTHORITY));
    let Ok(response) = router.oneshot(request).await;
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the mail service that reports the authority and origin it was given.
    fn echo() -> Router {
        Router::new().fallback(|request: Request<Body>| async move {
            let header = |name| {
                request
                    .headers()
                    .get(name)
                    .and_then(|v: &HeaderValue| v.to_str().ok())
                    .unwrap_or("-")
                    .to_owned()
            };
            format!("{} {}", header(header::HOST), header(header::ORIGIN))
        })
    }
    async fn text(response: Response<Body>) -> (StatusCode, String) {
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn window_requests_carry_the_service_origin_only_when_they_are_the_page_s_own() {
        let router: Shared = Arc::default();
        let _ = router.set(echo());
        let request = |origin: Option<&str>| {
            let mut builder = axum::http::Request::builder()
                .method("POST")
                .uri("colecto://localhost/api/v1/send?x=1");
            if let Some(origin) = origin {
                builder = builder.header(header::ORIGIN, origin);
            }
            builder.body(Vec::new()).unwrap()
        };
        for (origin, seen) in [
            (Some(PAGE_ORIGIN), ORIGIN),
            // WebKitGTK omits the header on the page's own requests.
            (None, ORIGIN),
            (Some("null"), "null"),
            (Some("https://evil.example"), "null"),
        ] {
            let response = serve(&router, request(origin)).await;
            assert_eq!(
                String::from_utf8(response.into_body()).unwrap(),
                format!("{AUTHORITY} {seen}"),
                "{origin:?}"
            );
        }
        // Event streams never go through the buffering scheme handler.
        let stream = axum::http::Request::builder()
            .uri("colecto://localhost/api/v1/events")
            .body(Vec::new())
            .unwrap();
        assert_eq!(serve(&router, stream).await.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_api_port_admits_api_keys_on_loopback_names_only() {
        let call = |host: &str, authorization: Option<&str>, origin: Option<&str>, path: &str| {
            let mut builder = Request::builder().uri(path).header(header::HOST, host);
            if let Some(value) = authorization {
                builder = builder.header(header::AUTHORIZATION, value);
            }
            if let Some(value) = origin {
                builder = builder.header(header::ORIGIN, value);
            }
            from_loopback(4199, echo(), builder.body(Body::empty()).unwrap())
        };
        let key = Some("Bearer colecto_abc.def");
        assert_eq!(
            text(call("127.0.0.1:4199", key, None, "/api/v1/mcp").await).await,
            (StatusCode::OK, format!("{AUTHORITY} -"))
        );
        assert_eq!(
            text(
                call(
                    "localhost:4199",
                    key,
                    Some("http://localhost:4199"),
                    "/api/v1/send"
                )
                .await
            )
            .await,
            (StatusCode::OK, format!("{AUTHORITY} {ORIGIN}"))
        );
        assert_eq!(
            text(
                call(
                    "127.0.0.1:4199",
                    key,
                    Some("https://evil.example"),
                    "/api/v1/send"
                )
                .await
            )
            .await,
            (StatusCode::OK, format!("{AUTHORITY} null"))
        );
        assert_eq!(
            text(call("127.0.0.1:4199", None, None, "/api/v1/version").await)
                .await
                .0,
            StatusCode::OK
        );
        for (host, authorization) in [
            (
                "127.0.0.1:4199",
                Some(&*format!("Bearer {}", "b".repeat(64))),
            ),
            ("127.0.0.1:4199", None),
            ("evil.example:4199", key),
            ("127.0.0.1:4200", key),
        ] {
            let response = call(host, authorization, None, "/api/v1/account").await;
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{host} {authorization:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_api_port_opens_and_closes_with_mcp() {
        let probe = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let api = ApiPort::new(port, echo());
        let connect = || tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port));
        assert!(connect().await.is_err());
        api.set(true).await;
        api.set(true).await;
        assert!(connect().await.is_ok());
        api.set(false).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(connect().await.is_err());
    }

    #[test]
    fn decode_utf8_chunk_reassembles_split_multibyte_sequences() {
        let input = "hello 世界 👋 end";
        let mut pending = Vec::new();
        let mut out = String::new();
        // Feed 1 byte at a time so every multibyte sequence crosses chunk boundaries.
        for byte in input.as_bytes() {
            out.push_str(&decode_utf8_chunk(&mut pending, std::slice::from_ref(byte)));
        }
        out.push_str(&flush_utf8(&mut pending));
        assert_eq!(out, input);
        assert!(pending.is_empty());

        // Invalid byte in the middle becomes U+FFFD while trailing valid UTF-8 survives.
        let mut mixed = decode_utf8_chunk(&mut pending, b"a\xff\xe4");
        mixed.push_str(&decode_utf8_chunk(&mut pending, b"\xb8\x96"));
        mixed.push_str(&decode_utf8_chunk(&mut pending, b"\xf0\x9f"));
        mixed.push_str(&flush_utf8(&mut pending));
        assert_eq!(mixed, "a\u{FFFD}世\u{FFFD}");
    }

    #[tokio::test]
    async fn streams_guards_against_close_all_and_same_id_replacement_races() {
        let streams = Streams::default();
        let before_close = streams.generation();
        streams.close_all();

        // A stream that connected before `close_all` must be aborted immediately on register.
        let stale = tokio::spawn(std::future::pending::<()>());
        let stale_handle = stale.abort_handle();
        assert!(streams.register(1, before_close, stale_handle).is_none());
        assert!(stale.await.unwrap_err().is_cancelled());

        // Replacing an open stream with the same id aborts the old one, and the old task's
        // cleanup must not remove the replacement.
        let generation = streams.generation();
        let first = tokio::spawn(std::future::pending::<()>());
        let first_token = streams
            .register(7, generation, first.abort_handle())
            .expect("registered first");
        let second = tokio::spawn(std::future::pending::<()>());
        let second_token = streams
            .register(7, generation, second.abort_handle())
            .expect("registered second");
        assert!(first.await.unwrap_err().is_cancelled());
        streams.remove_if_token(7, first_token);
        assert!(!second.is_finished());
        // Closing id 7 must still find and abort `second`.
        streams.close(7);
        assert!(second.await.unwrap_err().is_cancelled());
        streams.remove_if_token(7, second_token);
    }
}
