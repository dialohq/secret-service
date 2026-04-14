mod args;
mod db;
mod mode;
mod transport;
mod utils;

use args::Args;
use clap::Parser;
use tracing::{error, info};
use tracing_subscriber;

use crate::mode::ExecutionMode;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    info!("SecretService starting...");

    let result = match Args::parse().validate() {
        Ok(args) => args,
        Err(e) => {
            error!(err = ?e, "Error validating arguments");
            std::process::exit(1);
        }
    }
    .run()
    .await;

    match result {
        Ok(()) => {
            info!("Finished successfully")
        }
        Err(err) => {
            error!("Encountered an error: {}", err);
            std::process::exit(1);
        }
    }
}
