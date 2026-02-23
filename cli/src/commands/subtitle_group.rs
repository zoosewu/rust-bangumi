use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum SubtitleGroupAction {
    List,
    Add { name: String },
    Delete { id: i64 },
}

pub async fn run(_client: &ApiClient, _action: SubtitleGroupAction, _json: bool) -> Result<()> { todo!() }
