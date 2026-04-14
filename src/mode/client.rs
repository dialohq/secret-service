use super::ExecutionMode;
use crate::args::Args;
use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use gethostname::gethostname;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::info;

pub struct Client {
    bind: String,
    target: String,
}

impl Client {
    pub fn new(args: &Args) -> Result<Self> {
        let bind = match args.bind {
            None => bail!("Client mode requires --bind"),
            Some(ref b) => match b.parse::<Ipv4Addr>() {
                Err(err) => bail!("Error parsing ipv4 address ({}): {}", b, err),
                _ => b.clone(),
            },
        };
        let target = match args.target {
            None => bail!("Client mode requires --target"),
            Some(ref trgt) => match trgt.parse::<SocketAddr>() {
                Err(err) => bail!("Error parsing socket address ({}): {}", trgt, err),
                _ => trgt.clone(),
            },
        };

        Ok(Self { target, bind })
    }
}

#[async_trait]
impl ExecutionMode for Client {
    async fn run(&self) -> Result<()> {
        let hostname = gethostname()
            .into_string()
            .map_err(|_| anyhow!("hostname is not valid UTF-8"))?;
        info!(
            hostname = hostname,
            target = self.target,
            bind = self.bind,
            "Starting client"
        );

        let socket = Arc::new(UdpSocket::bind(format!("{}:41235", self.bind)).await?);
        socket.set_broadcast(true)?;

        let recv_s = socket.clone();
        let snd_s = socket.clone();

        let listener_handle = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            info!("Waiting for reply");
            loop {
                let (n, src) = recv_s.recv_from(&mut buf).await?;
                let src = src.to_string();
                let msg = std::str::from_utf8(&buf[..n])?;
                info!(msg, src, "Message received");
                break;
            }
            Ok::<(), anyhow::Error>(())
        });
        let target = self.target.clone();
        let discover_handle = tokio::spawn(async move {
            loop {
                info!(target, "Sending broadcast");
                snd_s.send_to(hostname.as_bytes(), &target).await?;
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            // Just for type inference
            #[allow(unreachable_code)]
            Ok::<(), anyhow::Error>(())
        });

        eprintln!("[client] waiting for either reply or continued discovery...");
        let _: Result<Result<()>> = tokio::select! {
            _ = discover_handle => {
                info!("discover handle finished");
                Ok(Ok(()))
            },
            _ = listener_handle => {
                info!("listener handle finished (got reply)");
                Ok(Ok(()))
            },
        };
        info!("shutting down");
        Ok(())
    }
}
