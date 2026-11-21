#[tokio::main]
async fn main() {
    colecto_service::logging::init();
    if let Err(error) = colecto_daemon::runtime::run().await {
        tracing::error!("{error}");
        std::process::exit(1);
    }
}
