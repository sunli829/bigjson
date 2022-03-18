use server::{create_server, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "bigjson=debug");
    }
    tracing_subscriber::fmt::init();

    let config = ServerConfig::parse();
    Ok(create_server(config)?.await?)
}
