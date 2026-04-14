mod ssh;

use std::collections::HashMap;

use crate::mode::Secret;
use anyhow::Result;
pub use ssh::SSHSession;

pub trait Transporter {
    async fn ensure_secrets(&self, secrets: &HashMap<String, Secret>) -> Result<()>;
}
