use super::Secret;
use crate::db::HostkeyDB;
use crate::transport::SSHSession;
use crate::transport::Transporter;
use anyhow::{Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::net::UdpSocket;
use tracing::{Instrument, Level, error, info, span};

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SSHAccessConfig {
    pub address: String,
    pub key: Secret,
    pub username: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AccessType {
    #[serde(rename = "ssh")]
    Ssh(SSHAccessConfig),
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct HostConfig {
    access: AccessType,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ConfigFile {
    hosts: HashMap<String, HostConfig>,
    secrets: HashMap<String, Secret>,
}

pub struct Server {
    port: u16,
    root: bool,
    db: Arc<HostkeyDB>,
    config: ConfigFile,
}

impl Server {
    pub async fn new(config: String, port: u16, root: bool) -> Result<Self> {
        info!(source = ?config, "Reading config file");
        let cfg_file = Path::new(&config);

        if !cfg_file.exists() {
            bail!("Config file does not exist: {}", config);
        }

        let cfg_json = match fs::read_to_string(cfg_file).await {
            Ok(contents) => {
                info!(size = contents.len(), "Read config file contents");
                contents
            }
            Err(err) => bail!("Couldn't read contents of config file {}: {}", &config, err),
        };
        let cfg: ConfigFile = match serde_json::from_str::<ConfigFile>(&cfg_json) {
            Ok(cfg) => {
                info!(
                    hosts = cfg.hosts.len(),
                    secrets = cfg.secrets.len(),
                    "Parsed config successfully"
                );
                cfg
            }
            Err(err) => bail!("Malformed JSON in config file {}: {}", config, err),
        };

        let state_dir = match std::env::var("STATE_DIRECTORY") {
            Ok(dir) => dir,
            Err(err) => bail!("Couldn't read $STATE_DIRECTORY env var: {}", err),
        };

        let db = HostkeyDB::new(state_dir).await?;
        db.init_hosts(cfg.hosts.keys()).await?;

        Ok(Self {
            port: port,
            root: root,
            db: Arc::new(db),
            config: cfg,
        })
    }
    async fn handle(&self, hostname: &str, host: &HostConfig) -> Result<()> {
        let span = span!(Level::INFO, "handling", host = hostname);
        match &host.access {
            AccessType::Ssh(ssh_config) => {
                let client = SSHSession::new(self.db.clone(), hostname, ssh_config, self.root)
                    .instrument(span.clone())
                    .await?;
                client
                    .ensure_secrets(&self.config.secrets)
                    .instrument(span)
                    .await?;
                Ok(())
            }
        }
    }
    pub async fn run(&self) -> Result<()> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", self.port)).await?;
        socket.set_broadcast(true)?;
        let mut buf = [0u8; 4096];
        loop {
            info!("Waiting for msg...");
            let (n, src) = match socket.recv_from(&mut buf).await {
                Ok((n, src)) => {
                    info!(?src, len = n, "Received message");
                    (n, src)
                }
                Err(err) => {
                    error!(?err, "Error receiving broadcast message");
                    continue;
                }
            };
            let msg = match std::str::from_utf8(&buf[..n]) {
                Ok(msg) => msg,
                Err(err) => {
                    error!(?err, "Error reading message contents");
                    continue;
                }
            };
            match self.config.hosts.get(msg) {
                Some(host) => {
                    if let Err(err) = self.handle(msg, &host).await {
                        error!(?err, host = msg, "Error handling host");
                    } else {
                        let reply_addr = std::net::SocketAddr::new(src.ip(), 41235);
                        info!(
                            ?reply_addr,
                            host = msg,
                            "Handled successfully, notifying client"
                        );
                        if let Err(err) = socket.send_to(b"ok", reply_addr).await {
                            error!(?err, "Failed to send reply to client");
                        }
                    }
                }
                None => {
                    error!(got = msg, "Couldn't find matching host.");
                }
            };
        }
    }
}
