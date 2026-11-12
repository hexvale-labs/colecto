//! Tauri shell: runs the mail service in-process on a loopback port and opens it in a WebView.
//! The UI is compiled into the binary, so a bundled app needs no files beside it.
use std::{net::Ipv4Addr, path::PathBuf, sync::Arc};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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

fn start() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = std::env::var("COLECTO_PORT")
        .unwrap_or_else(|_| "4173".into())
        .parse()?;
    let listener =
        tauri::async_runtime::block_on(tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)))?;
    let authority = listener.local_addr()?.to_string();
    tauri::Builder::default()
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
            let service = colecto_host::MailService::new(
                accounts,
                colecto_host::AccessPolicy::local(authority.clone()),
                assets,
            )?;
            let owner_token =
                zeroize::Zeroizing::new(colecto_host::runtime::local_owner_token(&directory)?);
            let origin = format!("http://{authority}");
            let initialization =
                colecto_host::runtime::owner_initialization_script(&origin, &owner_token);
            let router = service.router();
            let (stop, stopped) = tokio::sync::watch::channel(false);
            // Keep the sender alive for the app's lifetime; dropping it would stop syncing.
            app.manage(stop);
            tauri::async_runtime::spawn(
                service.run_background(std::time::Duration::from_secs(60), stopped),
            );
            tauri::async_runtime::spawn(async move {
                if let Err(error) = axum::serve(listener, router).await {
                    tracing::error!(%error, "Mail service stopped");
                }
            });
            tracing::info!(url = %origin, data = %directory.display(), "Colecto started");
            let navigation_authority = authority.clone();
            let window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(origin.parse()?),
            )
            .initialization_script(initialization)
            .on_navigation(move |destination| {
                colecto_host::runtime::native_navigation_allowed(&navigation_authority, destination)
            });
            #[cfg(desktop)]
            let window = window
                .title("Colecto")
                .inner_size(1200.0, 800.0)
                // Mail opens links in a new window (`<base target="_blank">`). Without a
                // handler WebKitGTK drops the request, so every link and button in a mail
                // did nothing; send them to the system browser instead.
                .on_new_window(|url, _| {
                    if colecto_host::runtime::external_link_allowed(&url) {
                        if let Err(error) = open::that_detached(url.as_str()) {
                            tracing::warn!(%error, "Could not open a link in the system browser");
                        }
                    }
                    tauri::webview::NewWindowResponse::Deny
                });
            window.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}
