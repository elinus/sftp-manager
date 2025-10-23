pub fn init_logging() {
    // tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();
    use tracing_subscriber::{EnvFilter, fmt};

    // Enable INFO logs and include HTTP tracing
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug"));

    fmt()
        .with_env_filter(filter)
        .with_target(false) // hides target module paths
        .compact() // cleaner output
        .init();
}
