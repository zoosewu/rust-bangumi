use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum DownloadAction {
    List,
}

pub async fn run(_client: &ApiClient, _action: DownloadAction, _json: bool) -> Result<()> { todo!() }
