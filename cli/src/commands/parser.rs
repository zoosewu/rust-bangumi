use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ParserAction {
    List,
    Show { id: i64 },
    Add,
    Update { id: i64 },
    Delete { id: i64 },
    Preview,
}

pub async fn run(_client: &ApiClient, _action: ParserAction, _json: bool) -> Result<()> { todo!() }
