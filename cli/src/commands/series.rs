use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum SeriesAction {
    List,
    Show { id: i64 },
    Update { id: i64 },
    Links { id: i64 },
}

pub async fn run(_client: &ApiClient, _action: SeriesAction, _json: bool) -> Result<()> { todo!() }
