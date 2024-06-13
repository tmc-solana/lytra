mod config;
mod tasks;
mod twitter;
mod ui;

use crate::config::{generate_keypair_if_not_exists, load_or_create_config};
use crate::tasks::{initialize_user_data, start_user_tasks};
use crate::ui::run_ui;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::{Keypair, Signer};
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_or_create_config("config.json");
    let keypair: Keypair = generate_keypair_if_not_exists("keypair.json");

    let rpc_client =
        RpcClient::new_with_commitment(config.rpc_url.clone(), CommitmentConfig::confirmed());
    let balance = rpc_client.get_balance(&keypair.pubkey()).await.unwrap();

    let user_data = Arc::new(Mutex::new(initialize_user_data(&config.users)));

    // let _ = twitter::login().await;

    start_user_tasks(Arc::clone(&user_data), &config.users);

    run_ui(keypair, balance, user_data, config).await?;

    Ok(())
}
