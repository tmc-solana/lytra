use regex::Regex;
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;
use std::{error::Error, str::FromStr, sync::Arc};

use crate::State;

pub async fn start_user_tasks(
    tweet: String,
    state: State,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let buy_config = tmc_solana_proto::proto::BuyConfig {
        slippage: state.config.buy_config.slippage,
        use_jito: state.config.buy_config.use_jito,
        jito_tip: state.config.buy_config.jito_tip,
        autobuy: false,
        prio_fee: 0.0001,
        sol_amount_left: 0.0,
        sol_amount_right: 0.0,
        sol_amount_autobuy: 0.0,
    };
    match find_solana_token_address(&tweet).await {
        Ok(option) => match option {
            Some(token) => {
                let market_res = identify_markets(&token).await;

                match market_res {
                    Ok(market) => match market {
                        "PumpFun" => {
                            let status = format!("Found PumpFun Token: {token}");
                            tokio::task::spawn(async move {
                                state
                                    .pumpfun_engine
                                    .buy(
                                        state.wallet.insecure_clone(),
                                        Pubkey::from_str(&token).unwrap(),
                                        state.config.buy_config.amount,
                                        state.config.buy_config.slippage,
                                        buy_config.clone(),
                                    )
                                    .await
                                    .unwrap();
                            });
                            return Ok(status);
                        }
                        _ => {
                            let status = format!("Found Jupiter Token: {token}");
                            tokio::task::spawn(async move {
                                tracing::info!("{token}");
                                state
                                    .jupiter_engine
                                    .buy(
                                        state.wallet.insecure_clone(),
                                        Pubkey::from_str(&token).unwrap(),
                                        state.config.buy_config.amount,
                                        state.config.buy_config.slippage,
                                        buy_config.clone(),
                                    )
                                    .await
                                    .unwrap();
                            });
                            return Ok(status);
                        }
                    },
                    Err(_) => {
                        return Ok("Error Occurred... Waiting for new Tweet".into());
                    }
                }
            }
            None => {
                return Ok("Waiting for new Tweet".into());
            }
        },
        Err(_) => {
            return Ok("Error Occurred... Waiting for new Tweet".into());
        }
    }
}

pub async fn sell_token_task(
    token: String,
    amount: f64,
    state: State,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let market_res = identify_markets(&token).await;

    let sell_config = tmc_solana_proto::proto::SellConfig {
        slippage: state.config.sell_config.slippage,
        use_jito: state.config.sell_config.use_jito,
        jito_tip: state.config.sell_config.jito_tip,
        prio_fee: 0.0001,
        sol_amount_left: 0.0,
        sol_amount_right: 0.0,
        ..Default::default()
    };

    match market_res {
        Ok(market) => match market {
            "PumpFun" => {
                tokio::task::spawn(async move {
                    state
                        .pumpfun_engine
                        .sell(
                            state.wallet.insecure_clone(),
                            Pubkey::from_str(&token).unwrap(),
                            amount,
                            state.config.sell_config.slippage,
                            sell_config.clone(),
                        )
                        .await
                        .unwrap();
                });
            }
            _ => {
                tokio::task::spawn(async move {
                    state
                        .jupiter_engine
                        .sell(
                            state.wallet.insecure_clone(),
                            Pubkey::from_str(&token).unwrap(),
                            amount,
                            state.config.sell_config.slippage,
                            sell_config.clone(),
                        )
                        .await
                        .unwrap();
                });
            }
        },
        Err(_) => {
            return Ok(());
        }
    }
    Ok(())
}

async fn expand_url(short_url: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let response = client.get(short_url).send().await?.error_for_status()?;
    let final_url = response.url().clone();
    Ok(final_url.to_string())
}

// Function to find a Solana token address in a tweet
async fn find_solana_token_address(
    tweet: &str,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    // Regular expression to match Solana token addresses
    let solana_address_regex = Regex::new(r"\b[A-HJ-NP-Za-km-z1-9]{32,44}\b")?;
    // Regular expression to match shortened URLs
    let tco_url_regex = Regex::new(r"https://t\.co/[A-Za-z0-9]+")?;

    // Check for any Solana token address directly in the tweet
    if let Some(matched) = solana_address_regex.find(tweet) {
        return Ok(Some(matched.as_str().to_string()));
    }

    // Check for any shortened URLs and expand them
    for short_url in tco_url_regex.find_iter(tweet) {
        let expanded_url = expand_url(short_url.as_str()).await?;
        if let Some(matched) = solana_address_regex.find(&expanded_url) {
            return Ok(Some(matched.as_str().to_string()));
        }
    }

    Ok(None)
}

pub async fn identify_markets(mint: &str) -> Result<&str, Box<dyn Error + Send + Sync>> {
    let response = reqwest::Client::builder()
        .build()?
        .get(format!("https://frontend-api.pump.fun/coins/{}", mint))
        .header("Accept", "application/json")
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::OK {
        let data: Value = response.json().await?;
        let is_raydium = !data["raydium_pool"].is_null();
        if is_raydium {
            return Ok("Raydium");
        }
        return Ok("PumpFun");
    }
    Ok("None")
}
