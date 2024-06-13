use rand::seq::SliceRandom;
use rand::thread_rng;
use regex::Regex;
use std::error::Error;
use std::sync::{Arc, Mutex};
use tokio::task;
use tokio::time::{self, Duration};

#[derive(Clone)]
pub struct UserInfo {
    pub username: String,
    pub last_tweet: String,
    pub status: String,
}

pub fn initialize_user_data(users: &[String]) -> Vec<UserInfo> {
    users
        .iter()
        .map(|username| UserInfo {
            username: username.clone(),
            last_tweet: "Fetching...".to_string(),
            status: "Initializing...".to_string(),
        })
        .collect()
}

pub fn start_user_tasks(user_data: Arc<Mutex<Vec<UserInfo>>>, users: &[String]) {
    let tweets = [
        "https://pump.fun/HDz6x7UcRwpFeg91n6MWkGkqbhnB4ggTjhTWZHArkjs1",
        "bro https://t.co/mFObpqHzOv",
        "swap 3sX3Gk9W539fxmdxqUMDyzkPVd3DiZ96FVUfRqbg22Ha",
        "pickel rich ZK8z8cvqpsGcxY6A5QdA7u1GAmdpcQudX9YFDq7pump",
        "efewffew

4QPp2fKk6Ta1Q1vKZSCyZCRWir5BY62r6CWEYvsUpump",
        "buying this: 4QPp2fKk6Ta1Q1vKZSCyZCRWir5BY62r6CWEYvsUpump",
        "eregregr

8qYH37jFCVbGSjQPdMsf8TDwp1JHTjU1McA8GoCCpump",
    ];
    let statuses = [
        "Found PumpFun Token",
        "Found Jupiter Token",
        "Waiting for new Tweet...",
        "Buying PumpFun Token...",
        "Buying Jupiter Token...",
        "Selling Jupiter Token...",
        "Selling PumpFun Token...",
    ];

    for (i, username) in users.iter().enumerate() {
        let user_data = Arc::clone(&user_data);
        let username = username.clone();
        task::spawn(async move {
            let tweet = tweets[i];
            time::sleep(Duration::from_secs(2)).await;
            let mut rng = thread_rng();

            if let Some(random_status) = statuses.choose(&mut rng) {
                let mut user_data = user_data.lock().unwrap();
                if let Some(user) = user_data.get_mut(i) {
                    user.last_tweet = tweet.into();
                    user.status = format!("{random_status}");
                }
            }

            // match find_solana_token_address(tweet).await {
            //     Ok(option) => match option {
            //         Some(token) => {
            //             let market_res = identify_markets(&token).await;
            //
            //             match market_res {
            //                 Ok(market) => match market {
            //                     "PumpFun" => {
            //                         let mut user_data = user_data.lock().unwrap();
            //                         if let Some(user) = user_data.get_mut(i) {
            //                             user.last_tweet = tweet.into();
            //                             user.status = format!("Found PumpFun Token: {token}");
            //                         }
            //                     }
            //                     _ => {
            //                         let mut user_data = user_data.lock().unwrap();
            //                         if let Some(user) = user_data.get_mut(i) {
            //                             user.last_tweet = tweet.into();
            //                             user.status = format!("Found Jupiter Token: {token}");
            //                         }
            //                     }
            //                 },
            //                 Err(_) => {
            //                     let mut user_data = user_data.lock().unwrap();
            //                     if let Some(user) = user_data.get_mut(i) {
            //                         user.last_tweet = tweet.into();
            //                         user.status = "Error Occurred... Waiting for new Tweet".into();
            //                     }
            //                 }
            //             }
            //         }
            //         None => {
            //             let mut user_data = user_data.lock().unwrap();
            //             if let Some(user) = user_data.get_mut(i) {
            //                 user.last_tweet = tweet.into();
            //                 user.status = "Waiting for new Tweet".into();
            //             }
            //         }
            //     },
            //     Err(_) => {
            //         let mut user_data = user_data.lock().unwrap();
            //         if let Some(user) = user_data.get_mut(i) {
            //             user.last_tweet = tweet.into();
            //             user.status = "Error Occurred... Waiting for new Tweet".into();
            //         }
            //     }
            // }
        });
    }
}

// Function to expand a shortened URL
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
        return Ok("PumpFun");
    }
    Ok("None")
}
