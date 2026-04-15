use anyhow::{Result, anyhow};
use gethostname::gethostname;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::info;

pub struct Client {
    bind: Ipv4Addr,
    port: u16,
    target: SocketAddr,
}

impl Client {
    pub fn new(bind: Ipv4Addr, target: SocketAddr, port: u16) -> Self {
        Self { target, bind, port }
    }
    pub async fn run(&self) -> Result<()> {
        let hostname = gethostname()
            .into_string()
            .map_err(|_| anyhow!("hostname is not valid UTF-8"))?;
        info!(?hostname, ?self.target, ?self.bind, "Starting client");

        let socket = Arc::new(UdpSocket::bind(format!("{}:{}", self.bind, self.port)).await?);
        socket.set_broadcast(true)?;
        let mut buf = [0u8; 4096];
        loop {
            info!(?self.target, "Sending broadcast");
            socket.send_to(hostname.as_bytes(), &self.target).await?;

            tokio::select! {
                Ok((n, src)) = socket.recv_from(&mut buf) => {
                    info!(len = n, src = %src, "Message received");
                    break Ok(());
                },
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    continue;
                }
            }
        }
    }
}
