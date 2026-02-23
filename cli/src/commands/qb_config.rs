use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum QbConfigAction {
    SetCredentials { user: String, password: String },
}

pub async fn run(_client: &ApiClient, _action: QbConfigAction, _json: bool) -> Result<()> { todo!() }
