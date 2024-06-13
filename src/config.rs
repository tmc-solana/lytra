use serde::{Deserialize, Serialize};
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub rpc_url: String,
    pub users: Vec<String>,
}

pub fn load_config(path: &str) -> Option<Config> {
    let config_str = fs::read_to_string(path).ok()?;
    serde_json::from_str(&config_str).ok()
}

pub fn save_config(config: &Config, path: &str) {
    let config_str = serde_json::to_string_pretty(config).unwrap();
    fs::write(path, config_str).unwrap();
}

pub fn get_user_input(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

pub fn load_or_create_config(path: &str) -> Config {
    match load_config(path) {
        Some(config) => config,
        None => {
            let rpc_url = get_user_input("Enter the Solana RPC URL: ");
            let users_raw = get_user_input("Enter users to monitor (space between each user): ");
            let users: Vec<String> = users_raw
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            let config = Config { rpc_url, users };
            save_config(&config, path);
            config
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
