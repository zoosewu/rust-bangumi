use crate::client::ApiClient;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum SubscriptionAction {
    List,
    Add { url: String },
    Show { id: i64 },
    Update { id: i64 },
    Delete { id: i64 },
}

pub async fn run(_client: &ApiClient, _action: SubscriptionAction, _json: bool) -> Result<()> { todo!() }
