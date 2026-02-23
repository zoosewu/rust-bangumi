use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ConflictAction {
    List,
    Resolve { id: i64 },
    ResolveLink { id: i64 },
}

pub async fn run(_client: &ApiClient, _action: ConflictAction, _json: bool) -> Result<()> { todo!() }
