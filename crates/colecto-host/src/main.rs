#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    colecto_host::logging::init();
    if std::env::args().nth(1).as_deref() == Some("notify") {
        return colecto_host::runtime::notify();
    }
    colecto_host::runtime::run().await
}
