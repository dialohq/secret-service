use crate::mode::{Client, ExecutionMode, Server};
use anyhow::{Result, bail};
use async_trait::async_trait;
use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum Mode {
    Server,
    Client,
}

impl ToString for Mode {
    fn to_string(&self) -> String {
        match self {
            Mode::Server => String::from("server"),
            Mode::Client => String::from("client"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, default_value_t = Mode::Client)]
    pub mode: Mode,

    #[arg(long)]
    pub target: Option<String>,

    #[arg(long)]
    pub bind: Option<String>,

    #[arg(long)]
    pub config: Option<String>,

    #[arg(long, default_value_t = 41234)]
    pub port: u16,

    #[arg(long, default_value_t = false)]
    pub root: bool,
}

pub enum RuntimeMode {
    Server(Server),
    Client(Client),
}

#[async_trait]
impl ExecutionMode for RuntimeMode {
    async fn run(&self) -> Result<()> {
        match self {
            Self::Server(s) => s.run().await,
            Self::Client(c) => c.run().await,
        }
    }
}

impl Args {
    pub fn validate(&self) -> Result<impl ExecutionMode> {
        if self.port == 0 {
            bail!("--port must be a non-zero value");
        }
        match self.mode {
            Mode::Server => Ok(RuntimeMode::Server(Server::new(self)?)),
            Mode::Client => Ok(RuntimeMode::Client(Client::new(self)?)),
        }
    }
}
