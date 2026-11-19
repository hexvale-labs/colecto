//! Tauri shell: runs the mail service in-process and opens it in a WebView. On desktop the
//! window reaches it through the `colecto://` scheme with no port open; phones use loopback.
//! The UI is compiled into the binary, so a bundled app needs no files beside it.
#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
use std::net::Ipv4Addr;
use std::{path::PathBuf, sync::Arc};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Some(command) = std::env::args().nth(1) {
        colecto_host::logging::init();
        let result = match command.as_str() {
            "daemon" => tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(colecto_host::runtime::run()),
            "notify" => Err(colecto_host::runtime::NOTIFY_MERGED.into()),
            _ => Err("Usage: colecto [daemon]".into()),
        };
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    // Wayland compositors find the window's icon through the desktop entry named after its
    // app_id, which GTK takes from the program name. Every Linux package installs
    // `Colecto.desktop`, so match it instead of the binary name `colecto`.
    #[cfg(target_os = "linux")]
    glib::set_prgname(Some("Colecto"));
    if let Err(error) = start() {
        colecto_host::logging::init();
        tracing::error!("Colecto could not start: {error}");
        std::process::exit(1);
    }
}

/// Desktop windows load Colecto from the `colecto://` scheme and no port is open (see
/// `desktop.rs`). Phones keep a loopback port: Android's WebView does not pass request
/// bodies to custom schemes.
fn start() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = std::env::var("COLECTO_PORT")
        .unwrap_or_else(|_| "4173".into())
        .parse()?;
    #[cfg(desktop)]
    let (listener, authority, page) = (
        None::<tokio::net::TcpListener>,
        desktop::AUTHORITY.to_owned(),
        format!("{}/", desktop::PAGE_ORIGIN),
    );
    #[cfg(mobile)]
    let (listener, authority, page) = {
        let listener = tauri::async_runtime::block_on(tokio::net::TcpListener::bind((
            Ipv4Addr::LOCALHOST,
            port,
        )))?;
        let authority = listener.local_addr()?.to_string();
        (
            Some(listener),
            authority.clone(),
            format!("http://{authority}/"),
        )
    };
    let page_origin = page.trim_end_matches('/').to_owned();
    #[cfg(desktop)]
    let router: desktop::Shared = Arc::default();
    let builder = tauri::Builder::default().plugin(tauri_plugin_colecto_native::init());
    #[cfg(desktop)]
    let builder = {
        let protocol = router.clone();
        let streams = Arc::new(desktop::Streams::default());
        let reload = streams.clone();
        builder
            .manage(router.clone())
            .manage(streams)
            .invoke_handler(tauri::generate_handler![
                desktop::stream,
                desktop::close_stream
            ])
            .register_asynchronous_uri_scheme_protocol(
                desktop::SCHEME,
                move |_, request, responder| {
                    let router = protocol.clone();
                    tauri::async_runtime::spawn(async move {
                        responder.respond(desktop::serve(&router, request).await);
                    });
                },
            )
            // A reload or navigation leaves the old page's event streams with no one to read
            // them, and each counts as an open window for notifications.
            .on_page_load(move |_, payload| {
                if payload.event() == tauri::webview::PageLoadEvent::Started {
                    reload.close_all();
                }
            })
    };
    builder
        .setup(move |app| {
            colecto_host::logging::init_with_files(app.path().app_log_dir().ok().as_deref());
            let directory = match std::env::var_os("COLECTO_DATA_DIR") {
                Some(directory) => PathBuf::from(directory),
                None => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(&directory)?;
            let resolver = app.asset_resolver();
            let assets = colecto_host::Assets::Embedded(Arc::new(move |path: &str| {
                resolver
                    .get(path.to_owned())
                    .map(|asset| (asset.bytes, asset.mime_type))
            }));
            let accounts = colecto_host::runtime::open_accounts(&directory)?;
            let mut policy = colecto_host::AccessPolicy::local(authority.clone());
            policy.service_command = Some(colecto_host::runtime::service_command()?);
            let service = colecto_host::MailService::new(accounts, policy, assets)?;
            let owner_token =
                zeroize::Zeroizing::new(colecto_host::runtime::local_owner_token(&directory)?);
            let mut initialization =
                colecto_host::runtime::owner_initialization_script(&page_origin, &owner_token);
            let (stop, stopped) = tokio::sync::watch::channel(false);
            // Keep the sender alive for the app's lifetime; dropping it would stop syncing.
            app.manage(stop);
            tauri::async_runtime::spawn(
                service
                    .clone()
                    .run_background(std::time::Duration::from_secs(60), stopped),
            );
            #[cfg(desktop)]
            {
                let _ = router.set(service.router());
                let api = Arc::new(desktop::ApiPort::new(port, service.router()));
                // Settings → API keys shows MCP clients where to connect.
                initialization.push_str(&format!(
                    "\nwindow.__COLECTO_API__ = {};",
                    serde_json::to_string(&api.origin())?
                ));
                let mut events = service.subscribe();
                let enabled = service.mcp_enabled();
                tauri::async_runtime::spawn(async move {
                    api.set(enabled).await;
                    loop {
                        match events.recv().await {
                            Ok(("mcp", data)) => api.set(data.contains("true")).await,
                            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                            Err(_) => break,
                        }
                    }
                });
            }
            if let Some(listener) = listener {
                let router = service.router();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = axum::serve(listener, router).await {
                        tracing::error!(%error, "Mail service stopped");
                    }
                });
            }
            tracing::info!(url = %page_origin, data = %directory.display(), "Colecto started");
            let navigation_origin = page_origin.clone();
            #[cfg(desktop)]
            let url = tauri::WebviewUrl::CustomProtocol(page.parse()?);
            #[cfg(mobile)]
            let url = tauri::WebviewUrl::External(page.parse()?);
            let window = tauri::WebviewWindowBuilder::new(app, "main", url)
                .initialization_script(initialization)
                .on_navigation(move |destination| {
                    colecto_host::runtime::native_navigation_allowed(
                        &navigation_origin,
                        destination,
                    )
                })
                // Without a handler WebKitGTK denies every permission request, so
                // `Notification.requestPermission()` failed before anyone was asked and Settings
                // reported notifications as blocked. The page is Colecto's own UI, and it only
                // asks when the owner turns desktop notifications on, so allow that one.
                // Everything else keeps the webview's default (denied).
                .on_permission_request(|_, kind| match kind {
                    tauri::webview::PermissionKind::Notifications => {
                        tauri::webview::PermissionResponse::Allow
                    }
                    _ => tauri::webview::PermissionResponse::Default,
                });
            #[cfg(desktop)]
            let window = window
                .title("Colecto")
                .inner_size(1200.0, 800.0)
                // Mail opens links in a new window (`<base target="_blank">`). Without a
                // handler WebKitGTK drops the request, so every link and button in a mail
                // did nothing; send them to the system browser instead.
                .on_new_window(|url, _| {
                    if colecto_host::runtime::external_link_allowed(&url)
                        && let Err(error) = open::that_detached(url.as_str())
                    {
                        tracing::warn!(%error, "Could not open a link in the system browser");
                    }
                    tauri::webview::NewWindowResponse::Deny
                });
            window.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}
