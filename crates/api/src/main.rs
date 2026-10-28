//! Run the local foundation API with automatic SQLite migrations.
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::var_os("PIPELINE_AUTOPSY_DATABASE")
            .unwrap_or_else(|| "runs/metadata.sqlite".into()),
    );
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent).await?;
    }
    let port: u16 = std::env::var("PIPELINE_AUTOPSY_PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()?;
    let pool = pipeline_autopsy_storage::open_database(&path).await?;
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("Pipeline Autopsy API: http://{}", listener.local_addr()?);
    println!("SQLite metadata: {}", path.display());
    axum::serve(listener, pipeline_autopsy_api::router(pool.clone()))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    pool.close().await;
    Ok(())
}
