mod db;
mod mode;
mod transport;
mod utils;

use tracing_subscriber;
use tracing::info;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use mode::{Client, Server};
use std::net::{Ipv4Addr, SocketAddr};

#[derive(Parser)]
#[command(name = "secret-service")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Client {
        #[arg(short, long)]
        bind: Ipv4Addr,

        #[arg(short, long, default_value_t = 41235)]
        port: u16,

        #[arg(short, long)]
        target: SocketAddr,
    },
    Server {
        #[arg(short, long)]
        config: String,

        #[arg(short, long, default_value_t = 41234)]
        port: u16,

        #[arg(short, long)]
        root: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    info!("SecretService starting...");

    let cli = Cli::parse();
    match cli.command {
        Commands::Client { bind, target, port } => {
            let client = Client::new(bind, target, port);
            client.run().await
        }
        Commands::Server { config, port, root } => {
            let server = Server::new(config, port, root)?;
            server.run().await
        }
    }
}
