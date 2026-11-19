fn main() -> Result<(), Box<dyn std::error::Error>> {
    colecto_host::logging::init();
    if std::env::args().nth(1).as_deref() == Some("--version") {
        match colecto_host::commit() {
            Some(commit) => println!("colecto-daemon {} ({commit})", colecto_host::VERSION),
            None => println!("colecto-daemon {}", colecto_host::VERSION),
        }
        return Ok(());
    }
    if std::env::args().nth(1).is_some() {
        return Err("Usage: colecto-daemon [--version]".into());
    }
    tokio::runtime::Runtime::new()?.block_on(colecto_host::runtime::run())
}
