use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum RawItemAction {
    List,
    Show { id: i64 },
    Reparse { id: i64 },
    Skip { id: i64 },
}

pub async fn run(_client: &ApiClient, _action: RawItemAction, _json: bool) -> Result<()> { todo!() }
