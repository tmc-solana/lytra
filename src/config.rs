use serde::{Deserialize, Serialize};
use solana_sdk::signature::Keypair;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuyConfig {
    pub amount: f64,
    pub slippage: f64,
    pub use_jito: bool,
    pub jito_tip: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SellConfig {
    pub slippage: f64,
    pub use_jito: bool,
    pub jito_tip: f64,
    pub auto_sell: bool,
    pub sell_at: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    pub users: Vec<String>,
    pub license: String,
    pub buy_config: BuyConfig,
    pub sell_config: SellConfig,
}

pub fn load_config(path: &str) -> Option<Config> {
    let config_str = fs::read_to_string(path).ok()?;
    serde_json::from_str(&config_str).ok()
}

pub fn load_or_create_config(path: &str) -> Config {
    match load_config(path) {
        Some(config) => config,
        None => {
            panic!("No config found");
        }
    }
}

pub fn generate_keypair_if_not_exists(file_path: &str) -> Keypair {
    if Path::new(file_path).exists() {
        let data = fs::read(file_path).expect("Unable to read file");
        Keypair::from_bytes(&data).expect("Unable to parse keypair")
    } else {
        let keypair = Keypair::new();
        let serialized = keypair.to_bytes();
        let mut file = fs::File::create(file_path).expect("Unable to create file");
        file.write_all(&serialized).expect("Unable to write data");
        keypair
    }
}
