use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use colecto_core::{Accounts, Error, MailStore, SecretStore};
use colecto_host::{AccessPolicy, MailService};
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
async fn remote_access_requires_token_even_with_host_identity_headers() {
    let dir = tempfile::tempdir().unwrap();
    let accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(NoSecrets)).unwrap();
    let service = MailService::new(
        accounts,
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
            .header("x-colecto-extension", "host")
            .header("x-colecto-account", "demo");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = service
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
    use colecto_host::runtime::OwnedSocket;
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
async fn stopped_background_worker_returns_without_polling_or_notifications() {
    let dir = tempfile::tempdir().unwrap();
    let accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(NoSecrets)).unwrap();
    let service = MailService::new(
        accounts,
        AccessPolicy::local("127.0.0.1:4173".into()),
        dir.path().join("assets"),
    )
    .unwrap();
    let (_, stop) = tokio::sync::watch::channel(true);
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        service.run_background(std::time::Duration::from_secs(60), stop),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn remote_origin_uses_browser_canonical_default_port() {
    let dir = tempfile::tempdir().unwrap();
    let accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(NoSecrets)).unwrap();
    let service = MailService::new(
        accounts,
        AccessPolicy::remote("https://MAIL.example.test:443/", "a".repeat(64)).unwrap(),
        dir.path().join("assets"),
    )
    .unwrap();
    let request = Request::builder()
        .uri("/api/v1/accounts")
        .header("host", "mail.example.test")
        .header("origin", "https://mail.example.test")
        .header("authorization", format!("Bearer {}", "a".repeat(64)))
        .header("x-colecto-extension", "host")
        .header("x-colecto-account", "demo")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        service.router().oneshot(request).await.unwrap().status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn remote_owner_validation_issues_no_cookie_and_every_request_requires_bearer() {
    let dir = tempfile::tempdir().unwrap();
    let accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(NoSecrets)).unwrap();
    let service = MailService::new(
        accounts,
        AccessPolicy::remote("https://mail.example.test", "a".repeat(64)).unwrap(),
        dir.path().join("assets"),
    )
    .unwrap();
    let bootstrap = |token: bool, cookie: Option<&str>| {
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/v1/session")
            .header("host", "mail.example.test")
            .header("origin", "https://mail.example.test");
        if token {
            req = req.header("authorization", format!("Bearer {}", "a".repeat(64)));
        }
        if let Some(cookie) = cookie {
            req = req.header("cookie", cookie);
        }
        req.body(Body::empty()).unwrap()
    };
    assert_eq!(
        service
            .router()
            .oneshot(bootstrap(false, None))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = service
        .router()
        .oneshot(bootstrap(true, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("set-cookie"));
    let cookie = "colecto_owner=legacy-cookie";
    assert_eq!(
        service
            .router()
            .oneshot(bootstrap(false, Some(cookie)))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let req = Request::builder()
        .uri("/api/v1/accounts")
        .header("host", "mail.example.test")
        .header("cookie", cookie)
        .header("x-colecto-extension", "host")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        service.router().oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn static_asset_roots_cannot_expose_the_private_data_directory_or_owner_token() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let accounts = Accounts::open(MailStore::open(&data).unwrap(), Arc::new(NoSecrets)).unwrap();
    assert!(
        MailService::new(
            accounts,
            AccessPolicy::local("localhost".into()),
            dir.path().into()
        )
        .is_err()
    );
}
