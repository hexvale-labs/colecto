//! Foreground runtime shared by the desktop shell and standalone daemon.
use crate::{AccessPolicy, MailService};
use colecto_core::{Accounts, Error, JmapAccount, MailAccount, MailStore, SecretStore};
use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub fn open_accounts(directory: &Path) -> Result<Accounts, Box<dyn std::error::Error>> {
    let mut store = MailStore::open(directory)?;
    let keys = [
        "COLECTO_IMAP_HOST",
        "COLECTO_IMAP_PORT",
        "COLECTO_SMTP_HOST",
        "COLECTO_SMTP_PORT",
        "COLECTO_MAIL_USERNAME",
        "COLECTO_MAIL_PASSWORD",
        "COLECTO_MAIL_ADDRESS",
    ];
    let jmap_keys = ["COLECTO_JMAP_SESSION_URL", "COLECTO_JMAP_TOKEN"];
    let jmap = jmap_keys.iter().any(|key| env::var_os(key).is_some());
    let imap = keys[..6].iter().any(|key| env::var_os(key).is_some());
    if jmap && imap {
        return Err("Configure either JMAP or IMAP/SMTP, not both".into());
    }
    if jmap {
        let required = |key: &str| env::var(key).map_err(|_| format!("Missing {key}"));
        store.connect_jmap(JmapAccount {
            session_url: required("COLECTO_JMAP_SESSION_URL")?,
            token: required("COLECTO_JMAP_TOKEN")?,
            address: required("COLECTO_MAIL_ADDRESS")?,
        })?;
    } else if keys.iter().any(|key| env::var_os(key).is_some()) {
        let required = |key: &str| env::var(key).map_err(|_| format!("Missing {key}"));
        store.connect_account(MailAccount {
            imap_host: required("COLECTO_IMAP_HOST")?,
            imap_port: env::var("COLECTO_IMAP_PORT")
                .unwrap_or_else(|_| "993".into())
                .parse()?,
            smtp_host: required("COLECTO_SMTP_HOST")?,
            smtp_port: env::var("COLECTO_SMTP_PORT")
                .unwrap_or_else(|_| "587".into())
                .parse()?,
            username: required("COLECTO_MAIL_USERNAME")?,
            password: required("COLECTO_MAIL_PASSWORD")?,
            address: required("COLECTO_MAIL_ADDRESS")?,
        })?;
    }
    let namespace = format!("Colecto:{}", std::fs::canonicalize(directory)?.display());
    Ok(Accounts::open(store, Arc::new(SystemSecrets(namespace)))?)
}

struct SystemSecrets(String);
fn credential_error(_: keyring::Error) -> Error {
    Error::Invalid("System credential store unavailable or locked. Unlock your desktop keychain, or enable master password protection before adding accounts.".into())
}
impl SystemSecrets {
    fn entry(&self, id: &str) -> Result<keyring::Entry, Error> {
        if !cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows",
            target_os = "freebsd",
            target_os = "openbsd"
        )) {
            return Err(Error::Invalid("System credential storage is not supported on this platform; use master password protection".into()));
        }
        keyring::Entry::new(&self.0, id).map_err(credential_error)
    }
}
impl SecretStore for SystemSecrets {
    fn get(&self, id: &str) -> Result<String, Error> {
        self.entry(id)?.get_password().map_err(credential_error)
    }
    fn set(&self, id: &str, value: &str) -> Result<(), Error> {
        self.entry(id)?
            .set_password(value)
            .map_err(credential_error)
    }
    fn delete(&self, id: &str) -> Result<(), Error> {
        match self.entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(credential_error(e)),
        }
    }
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let directory = env::var_os("COLECTO_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".colecto-demo"));
    let assets = env::var_os("COLECTO_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist"));
    let interval = env::var("COLECTO_SYNC_SECONDS")
        .unwrap_or_else(|_| "60".into())
        .parse::<u64>()?;
    if interval < 10 {
        return Err("COLECTO_SYNC_SECONDS must be at least 10".into());
    }
    let bind = env::var("COLECTO_BIND")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse::<std::net::IpAddr>()?;
    let port = env::var("COLECTO_PORT")
        .unwrap_or_else(|_| "4173".into())
        .parse::<u16>()?;
    let socket = env::var_os("COLECTO_SOCKET").map(PathBuf::from);
    let remote = env::var("COLECTO_PUBLIC_ORIGIN").ok();
    if !bind.is_loopback() && remote.is_none() {
        return Err(
            "Non-loopback listeners require COLECTO_PUBLIC_ORIGIN and COLECTO_TOKEN_FILE".into(),
        );
    }
    if socket.is_some() && remote.is_some() {
        return Err("Use a local Unix socket or remote HTTPS origin, not both".into());
    }
    let listener = if socket.is_none() {
        Some(tokio::net::TcpListener::bind((bind, port)).await?)
    } else {
        None
    };
    let authority = listener
        .as_ref()
        .map(|l| l.local_addr().map(|a| a.to_string()))
        .transpose()?
        .unwrap_or_else(|| "localhost".into());
    let policy = match remote {
        Some(origin) => AccessPolicy::remote(
            &origin,
            read_token(&PathBuf::from(
                env::var_os("COLECTO_TOKEN_FILE")
                    .ok_or("Remote mode requires COLECTO_TOKEN_FILE")?,
            ))?,
        )?,
        None => AccessPolicy::local(authority.clone()),
    };
    let accounts = open_accounts(&directory)?;
    let service = MailService::new(accounts, policy, assets)?;
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(
        service
            .clone()
            .run_background(Duration::from_secs(interval), stopped),
    );
    let shutdown = async move {
        shutdown_signal().await;
        let _ = stop.send(true);
    };
    if let Some(listener) = listener {
        println!("Colecto local demo: http://{authority}");
        println!("Data directory: {}", directory.display());
        axum::serve(listener, service.router())
            .with_graceful_shutdown(shutdown)
            .await?;
    } else {
        #[cfg(unix)]
        {
            let socket = OwnedSocket::bind(socket.as_ref().unwrap())?;
            println!(
                "Colecto HTTP socket: {} (Host: localhost)",
                socket.path.display()
            );
            axum::serve(socket.listener()?, service.router())
                .with_graceful_shutdown(shutdown)
                .await?;
        }
        #[cfg(not(unix))]
        {
            return Err("Unix sockets are unavailable on this platform".into());
        }
    }
    worker.await?;
    Ok(())
}

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
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(unix)]
pub struct OwnedSocket {
    path: PathBuf,
    listener: std::os::unix::net::UnixListener,
    identity: (u64, u64),
}
#[cfg(unix)]
impl OwnedSocket {
    pub fn bind(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or("Socket requires an explicit private parent directory")?;
        let metadata = std::fs::metadata(parent)?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err("Socket directory must be private (chmod 700)".into());
        }
        let listener = std::os::unix::net::UnixListener::bind(path)?;
        let metadata = std::fs::symlink_metadata(path)?;
        let socket = Self {
            path: path.into(),
            listener,
            identity: (metadata.dev(), metadata.ino()),
        };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        socket.listener.set_nonblocking(true)?;
        Ok(socket)
    }
    pub fn listener(&self) -> Result<tokio::net::UnixListener, std::io::Error> {
        tokio::net::UnixListener::from_std(self.listener.try_clone()?)
    }
}
#[cfg(unix)]
impl Drop for OwnedSocket {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if std::fs::symlink_metadata(&self.path).is_ok_and(|m| (m.dev(), m.ino()) == self.identity)
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Load the OS-user owner's local bootstrap credential without exposing it over HTTP.
pub fn local_owner_token(directory: &Path) -> Result<String, Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let handle = std::fs::File::open(directory)?;
        let metadata = handle.metadata()?;
        if !metadata.is_dir() || metadata.uid() != rustix::process::geteuid().as_raw() {
            return Err("Local data directory must be owned by the current OS user".into());
        }
        handle.set_permissions(std::fs::Permissions::from_mode(0o700))?;
    }
    let path = directory.join("owner-token");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(crate::api_keys::random()?.as_bytes())?;
            file.sync_all()?;
            #[cfg(unix)]
            std::fs::File::open(directory)?.sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let token = read_token(&path)?;
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid local owner-token file; stop the service and restore or remove the file before restarting".into());
    }
    Ok(token)
}

pub fn owner_initialization_script(origin: &str, token: &str) -> String {
    format!(
        "if (window.location.origin === {}) {{ window.sessionStorage.setItem('colecto.token', {}); }}",
        serde_json::to_string(origin).expect("serialize origin"),
        serde_json::to_string(token).expect("serialize owner credential")
    )
}
