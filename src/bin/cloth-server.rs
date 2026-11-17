//! Local HTTP API for the React studio. CPU only, loopback only.
//!
//! cargo run --bin cloth-server
//! CLOTH_SERVER_ADDR=127.0.0.1:8787
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address: std::net::SocketAddr = std::env::var("CLOTH_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_owned())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("cloth-server listening on http://{address}");
    axum::serve(listener, marvelous_cloth::api::router()?).await?;
    Ok(())
}
