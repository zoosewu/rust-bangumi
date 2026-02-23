use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum FilterAction {
    List,
    Add,
    Delete { id: i64 },
    Preview,
}

pub async fn run(_client: &ApiClient, _action: FilterAction, _json: bool) -> Result<()> { todo!() }
