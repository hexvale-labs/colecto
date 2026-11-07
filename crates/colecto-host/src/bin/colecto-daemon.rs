fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("notify") {
        let origin = std::env::var("COLECTO_PUBLIC_ORIGIN")?;
        let token_file = std::path::PathBuf::from(
            std::env::var_os("COLECTO_TOKEN_FILE").ok_or("COLECTO_TOKEN_FILE is required")?,
        );
        return colecto_host::notifications::run_agent(&origin, &token_file);
    }
    if std::env::args().nth(1).is_some() {
        return Err("Usage: colecto-daemon [notify]".into());
    }
    tokio::runtime::Runtime::new()?.block_on(colecto_host::runtime::run())
}
