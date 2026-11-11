#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    colecto_host::logging::init();
    colecto_host::runtime::run().await
}
