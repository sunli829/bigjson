use std::path::PathBuf;

use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Parser)]
#[clap(author, version, about)]
pub struct ServerConfig {
    #[clap(long)]
    pub(crate) data_dir: Option<PathBuf>,
    #[clap(long, default_value = "127.0.0.1:3000")]
    pub(crate) bind: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            bind: "127.0.0.1:3000".to_string(),
        }
    }
}

impl ServerConfig {
    #[must_use]
    pub fn data_dir(self, path: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: Some(path.into()),
            ..self
        }
    }

    pub fn parse() -> Self {
        Parser::parse()
    }
}
