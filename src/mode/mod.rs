mod client;
mod server;

pub use client::Client;
pub use server::SSHAccessConfig;
pub use server::Server;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SecretType {
    Command,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Secret {
    pub command: String,
    pub target_path: String,

    #[allow(dead_code)]
    pub r#type: SecretType,
}
