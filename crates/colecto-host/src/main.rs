#[cfg(feature = "native")]
use std::{env, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(feature = "native"))]
    return colecto_host::runtime::run().await;
    #[cfg(feature = "native")]
    {
        let port: u16 = env::var("COLECTO_PORT")
            .unwrap_or_else(|_| "4173".into())
            .parse()?;
        let directory = env::var_os("COLECTO_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".colecto-demo"));
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist");
        if !assets.join("index.html").is_file() {
            return Err("Frontend build missing; run just ui-build first".into());
        }
        let accounts = colecto_host::runtime::open_accounts(&directory)?;
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
        let authority = listener.local_addr()?.to_string();
        println!("Colecto local demo: http://{authority}");
        println!("Draft directory: {}", directory.display());
        let service = colecto_host::MailService::new(
            accounts,
            colecto_host::AccessPolicy::local(authority.clone()),
            assets,
        )?;
        let owner_token =
            zeroize::Zeroizing::new(colecto_host::runtime::local_owner_token(&directory)?);
        let initialization = colecto_host::runtime::owner_initialization_script(
            &format!("http://{authority}"),
            &owner_token,
        );
        let application = service.router();
        let (_stop, stop) = tokio::sync::watch::channel(false);
        tokio::spawn(service.run_background(std::time::Duration::from_secs(60), stop));
        #[cfg(not(feature = "native"))]
        axum::serve(listener, application)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        #[cfg(feature = "native")]
        {
            tokio::spawn(async move {
                if let Err(error) = axum::serve(listener, application).await {
                    eprintln!("Host server failed: {error}");
                }
            });
            let url = format!("http://{authority}").parse()?;
            tauri::Builder::default()
                .setup(move |app| {
                    tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::External(url))
                        .initialization_script(initialization)
                        .title("Colecto")
                        .inner_size(1200.0, 800.0)
                        .on_navigation(move |destination| {
                            destination
                                .as_str()
                                .starts_with(&format!("http://{authority}/"))
                                || (destination.scheme() == "https"
                                    && destination.username().is_empty()
                                    && destination.password().is_none())
                        })
                        .build()?;
                    Ok(())
                })
                .run(tauri::generate_context!())?;
        }

        Ok(())
    }
}
