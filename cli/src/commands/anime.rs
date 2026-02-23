use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum AnimeAction {
    List,
    Add { title: String },
    Delete { id: i64 },
    Series { anime_id: i64 },
}

pub async fn run(_client: &ApiClient, _action: AnimeAction, _json: bool) -> Result<()> { todo!() }
