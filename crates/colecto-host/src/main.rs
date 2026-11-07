use colecto_core::MailStore;
use std::{env, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let store = MailStore::open(&directory)?;
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let authority = listener.local_addr()?.to_string();
    println!("Colecto local demo: http://{authority}");
    println!("Draft directory: {}", directory.display());
    axum::serve(listener, colecto_host::app(store, authority, assets))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
