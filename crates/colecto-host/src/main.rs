#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    colecto_host::logging::init();
    if std::env::args().nth(1).as_deref() == Some("notify") {
        return Err(colecto_host::runtime::NOTIFY_MERGED.into());
    }
    colecto_host::runtime::run().await
}
