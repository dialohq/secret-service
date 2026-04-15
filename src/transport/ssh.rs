use crate::db::HostkeyDB;
use crate::utils::run_command;
use crate::{
    mode::{SSHAccessConfig, Secret},
    transport::Transporter,
    utils,
};
use anyhow::{Context, Result, bail};
use russh::keys::{PrivateKeyWithHashAlg, decode_secret_key};
use russh::{
    ChannelId,
    client::Session,
    client::{Handle, Handler},
    keys::ssh_key,
};
use russh_sftp::client::SftpSession;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{instrument, Instrument, Level, event, info, span};
use uuid::Uuid;

#[derive(Clone)]
pub struct SSHClient {
    pub db_conn: Arc<HostkeyDB>,
    pub hostname: String,
}

impl Handler for SSHClient {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key
            .fingerprint(ssh_key::HashAlg::Sha256)
            .to_string();
        Ok(self
            .db_conn
            .authenticate(self.hostname.as_ref(), fingerprint))
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        _data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub struct SSHSession {
    pub sftp: SftpSession,
    pub session: Handle<SSHClient>,
    pub root: bool,

    #[allow(dead_code)]
    pub client: SSHClient,
}

impl SSHSession {
    async fn exec(&self, cmd: &str) -> Result<()> {
        let span = span!(Level::INFO, "exec-ssh", cmd);

        let mut channel = self.session.channel_open_session().await?;
        channel.exec(true, cmd).await?;
        let mut output = Vec::new();
        let mut exit_code = None;
        while let Some(msg) = channel.wait().await {
            span.in_scope(|| match msg {
                russh::ChannelMsg::Data { data } => {
                    info!(len = data.len(), "stdout data received");
                    output.extend_from_slice(&data);
                }
                russh::ChannelMsg::ExtendedData { data, .. } => {
                    let data_str: String = String::from_utf8_lossy(&data).into();
                    info!(data = data_str, "stderr data received");
                }
                russh::ChannelMsg::ExitStatus { exit_status } => {
                    info!(code = exit_status, "Command exited");
                    exit_code = Some(exit_status);
                }
                other => {
                    let msg = format!("{:?}", other);
                    event!(Level::DEBUG, msg, "Channel message received");
                }
            });
        }

        match exit_code {
            Some(0) => Ok(()),
            Some(code) => bail!(
                "'{}' failed (exit {}): {}",
                cmd,
                code,
                String::from_utf8_lossy(&output)
            ),
            None => bail!("'{}' exited without status", cmd),
        }
    }

    pub async fn ensure_directory(&self, path: PathBuf) -> Result<()> {
        let dir_str = path.to_string_lossy();
        if self.root {
            self.exec(&format!("sudo mkdir -p {}", dir_str)).await
                .with_context(|| format!("sudo mkdir -p failed: {}", dir_str))?;
        } else {
            let ancestors: Vec<_> = path.ancestors().collect();
            for dir in ancestors.into_iter().rev().skip(1) {
                let dir_str = dir.to_string_lossy();
                if self.sftp.metadata(dir_str.as_ref()).await.is_err() {
                    self.sftp
                        .create_dir(dir_str.as_ref())
                        .await
                        .with_context(|| format!("create_dir failed: {}", dir_str))?;
                }
            }
        }
        Ok(())
    }

    pub async fn ensure_secret(&self, secret: &Secret) -> Result<()> {
        let path = Path::new(&secret.target_path);
        let secret_name = match path.file_name() {
            Some(name) => name,
            None => bail!("Error extracting name from secret target path"),
        };
        let tmp_path = Path::new(&format!("/tmp/{}", Uuid::new_v4())).join(Path::new(secret_name));
        let tmp_dir = tmp_path.parent().unwrap().to_string_lossy().to_string();
        let tmp_path_str = tmp_path.into_os_string().into_string();
        // All components of this path are created from strings
        // I don't think this can fail realistically
        let tmp_path_str = tmp_path_str.expect(&format!(
            "Couldn't convert secret tmp path back into string"
        ));
        let secret_data = run_command("get-secret", &secret.command)?;

        self.sftp
            .create_dir(&tmp_dir)
            .await
            .with_context(|| format!("create_dir failed: {}", tmp_dir))?;
        self.sftp
            .create(&tmp_path_str)
            .await
            .with_context(|| format!("sftp create failed: {}", tmp_path_str))?;
        self.sftp
            .write(&tmp_path_str, secret_data.as_bytes())
            .await
            .with_context(|| format!("sftp write failed: {}", tmp_path_str))?;

        let sudo = if self.root { "sudo " } else { "" };

        self.exec(&format!("{}mv {} {}", sudo, tmp_path_str, &secret.target_path))
            .await
            .with_context(|| format!("mv to {} failed", secret.target_path))?;

        self.sftp
            .remove_dir(&tmp_dir)
            .await
            .with_context(|| format!("remove_dir failed: {}", tmp_dir))?;

        self.exec(&format!("{}chmod 0400 {}", sudo, &secret.target_path))
            .await
            .with_context(|| format!("chmod failed: {}", secret.target_path))?;

        if self.root {
            self.exec(&format!("sudo chown root:root {}", &secret.target_path))
                .await
                .with_context(|| format!("chown failed: {}", secret.target_path))?;
        }

        info!(
            secret = format!("{:?}", secret_name),
            secret.target_path, "secret placed at target path"
        );

        Ok(())
    }

    #[instrument(name = "ssh-connection", skip(db, config), fields(addr = config.address, username = config.username))]
    pub async fn new(
        db: Arc<HostkeyDB>,
        hostname: &str,
        config: &SSHAccessConfig,
        root: bool,
    ) -> Result<Self> {
        let encoded_key = utils::run_command("ssh-key", &config.key.command)?;
        let key = decode_secret_key(&encoded_key, None)?;
        let key = PrivateKeyWithHashAlg::new(Arc::new(key), None);

        let client = SSHClient {
            db_conn: db.clone(),
            hostname: hostname.into(),
        };
        let ssh_config = russh::client::Config::default();
        let mut session = russh::client::connect(
            Arc::new(ssh_config),
            (config.address.as_str(), 22),
            client.clone(),
        )
        .await?;
        let authentication_result = session
            .authenticate_publickey(config.username.clone(), key)
            .await?;
        if !authentication_result.success() {
            bail!("Authentication failure.");
        }
        let sftp_channel = session
            .channel_open_session()
            .await?;
        sftp_channel
            .request_subsystem(true, "sftp")
            .await?;
        let sftp = SftpSession::new(sftp_channel.into_stream())
            .await?;

        Ok(Self {
            client,
            sftp,
            session,
            root,
        })
    }
}

impl Transporter for SSHSession {
    async fn ensure_secrets(&self, secrets: &HashMap<String, Secret>) -> Result<()> {
        for (name, secret) in secrets.into_iter() {
            let span = span!(Level::INFO, "ensuring-secret", name);
            let path = PathBuf::from(secret.target_path.clone());
            if let Some(parent) = path.parent() {
                self.ensure_directory(parent.to_path_buf())
                    .instrument(span.clone())
                    .await?;
            }
            self.ensure_secret(secret).instrument(span).await?;
        }
        Ok(())
    }
}
