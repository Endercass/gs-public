use std::sync::Arc;

use giggleshitter_common::{error::Result, serve, state::Config};
use scorched::{logf, LogData, LogImportance};
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    logf!(
        Info,
        "Loading config from file: {}",
        confy::get_configuration_file_path("weirdproxy", None)?.display()
    );

    let config: Arc<Config> = Arc::new(confy::load("weirdproxy", None)?);

    serve(config, shutdown_signal()).await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
