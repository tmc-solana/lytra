mod config;
mod tasks;
mod twitter;
mod ui;

use crate::config::{generate_keypair_if_not_exists, load_or_create_config};
use crate::ui::run_ui;
use config::Config;
use reqwest::Client;
use reqwest_cookie_store::CookieStoreMutex;
use serde_json::{json, Value};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::nonblocking::tpu_client::TpuClient;
use solana_client::tpu_client::TpuClientConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::Keypair;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};
use tmc_solana_engine::jupiter::JupiterEngine;
use tmc_solana_engine::pumpfun::PumpFunEngine;
use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use ui::UserInfo;

pub struct State {
    config: Config,
    wallet: Keypair,
    jupiter_engine: JupiterEngine,
    pumpfun_engine: PumpFunEngine,
}

impl Clone for State {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            wallet: self.wallet.insecure_clone(),
            jupiter_engine: self.jupiter_engine.clone(),
            pumpfun_engine: self.pumpfun_engine.clone(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a file appender that rotates daily
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "app.log");
    let (file_writer, _file_guard): (NonBlocking, WorkerGuard) =
        tracing_appender::non_blocking(file_appender);

    // Create a stdout logger
    let (_stdout_writer, _stdout_guard): (NonBlocking, WorkerGuard) =
        tracing_appender::non_blocking(std::io::stdout());

    // Create a tracing subscriber that logs to both stdout and the file with level set to INFO
    let subscriber = tracing_subscriber::registry()
        // .with(
        //     fmt::layer()
        //         .with_writer(stdout_writer)
        //         .with_filter(LevelFilter::INFO),
        // )
        .with(
            fmt::layer()
                .with_writer(file_writer)
                .with_filter(LevelFilter::INFO),
        );

    // Set the global default subscriber
    tracing::subscriber::set_global_default(subscriber)?;

    // TEST: tui_logger
    tui_logger::init_logger(log::LevelFilter::Trace)?;
    tui_logger::set_default_level(log::LevelFilter::Trace);

    let config = load_or_create_config("config.json");
    let keypair: Keypair = generate_keypair_if_not_exists("keypair.json");
    tracing::info!("{}", keypair.to_base58_string());

    auth(config.license.clone()).await?;
    log::info!(target:"app", "Logged in!");

    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        config.rpc_url.clone(),
        CommitmentConfig::confirmed(),
    ));

    let (sender, receiver): (Sender<Vec<UserInfo>>, Receiver<Vec<UserInfo>>) = mpsc::channel();
    let jupiter_engine = tmc_solana_engine::jupiter::JupiterEngine {
        rpc_client: rpc_client.clone(),
    };
    let pumpfun_engine = tmc_solana_engine::pumpfun::PumpFunEngine {
        tpu_client: Arc::new(
            TpuClient::new(
                "tpu_client",
                rpc_client.clone(),
                "wss://api.mainnet-beta.solana.com",
                TpuClientConfig { fanout_slots: 10 },
            )
            .await
            .unwrap(),
        ),
    };

    let state = State {
        config: config.clone(),
        wallet: keypair,
        jupiter_engine,
        pumpfun_engine,
    };

    let state_cloned = state.clone();
    tokio::spawn(async move {
        let cookie_store = Arc::new(CookieStoreMutex::default());
        twitter::monitor(sender, cookie_store, state_cloned)
            .await
            .unwrap();
    });
    run_ui(state.wallet.insecure_clone(), state, receiver, rpc_client).await?;

    Ok(())
}

async fn auth(license: String) -> Result<(), Box<dyn std::error::Error>> {
    let hwid = machine_uid::get()?;

    let client = reqwest::Client::builder().build()?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Content-Type", "application/vnd.api+json".parse()?);
    headers.insert("Accept", "application/vnd.api+json".parse()?);
    let data: Value = json!({
        "meta": {
            "key": license,
            "scope": {
                "product": "7f122c90-2e52-4da8-be7e-ae76a8e0c35c",
                "policy": "23145965-850a-4ab5-8afa-aecc11d3ff37",
                "fingerprint": hwid,
            }
        }
    });

    let res = client
        .request(
            reqwest::Method::POST,
            "https://api.keygen.sh/v1/accounts/nidalee-party/licenses/actions/validate-key",
        )
        .headers(headers)
        // .body(data);
        .json(&data)
        .send()
        .await?;

    let body: Value = res.json().await?;

    let valid = body["meta"]["valid"].as_bool().unwrap_or(false);
    let code = body["meta"]["code"].as_str().unwrap_or("NO CODE");
    let license_id = body["data"]["id"].as_str().unwrap_or("");

    if code == "NO_MACHINES" {
        activate(&client, hwid, license.to_string(), license_id.to_string()).await?;
        println!("Machine got activated... Please restart the application!");
        std::process::exit(0);
    }

    if !valid {
        panic!("License is invaldid! Please contact the support team | reason: {code}");
    }

    Ok(())
}

async fn activate(
    client: &Client,
    hwid: String,
    license: String,
    license_id: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let data: Value = json!({
      "data": {
        "type": "machines",
        "attributes": {
          "fingerprint": hwid,
        },
        "relationships": {
          "license": {
            "data": {
              "type": "licenses",
              "id": license_id,
            }
          }
        }
      }
    });

    let mut headers = reqwest::header::HeaderMap::new();
    let bearer = format!("License {license}");
    headers.insert("Content-Type", "application/vnd.api+json".parse()?);
    headers.insert("Accept", "application/vnd.api+json".parse()?);
    headers.insert("Authorization", bearer.parse()?);

    let _res = client
        .request(
            reqwest::Method::POST,
            "https://api.keygen.sh/v1/accounts/nidalee-party/machines",
        )
        .headers(headers)
        .json(&data)
        .send()
        .await?;

    Ok(())
}
