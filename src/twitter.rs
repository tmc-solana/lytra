use std::{
    collections::HashMap,
    error::Error,
    str::FromStr,
    sync::{mpsc::Sender, Arc},
    time::Duration,
};

use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, COOKIE, REFERER, USER_AGENT},
    Client, Url,
};
use reqwest_cookie_store::CookieStoreMutex;
use serde_json::{json, Value};
use tokio::time::{sleep, Instant};

use crate::{tasks, ui::UserInfo, State};

pub async fn monitor(
    tx: Sender<Vec<UserInfo>>,
    cookie_store: Arc<CookieStoreMutex>,
    state: State,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let users = state.clone().config.users;

    let client = Client::builder()
        .cookie_store(true)
        .cookie_provider(cookie_store.clone())
        .build()?;

    let mut headers = get_headers();

    tracing::info!("Starting log in");
    log::warn!(target:"app", "Logging in Twitter!");
    login(&client, &mut headers).await?;
    log::info!(target:"app", "Logged in Twitter!");
    tracing::info!("Logged in");

    headers.insert(
        "X-Twitter-Auth-Type",
        HeaderValue::from_static("OAuth2Session"),
    );
    headers.insert("X-Twitter-Active-User", HeaderValue::from_static("yes"));

    let (cookie_header, csrf_token) = {
        let cookie_jar = cookie_store.lock().unwrap();
        let cookies = cookie_jar.get_request_values(&Url::from_str("https://twitter.com").unwrap());
        let cookie_header: String = cookies
            .into_iter()
            .map(|c| format!("{}={}", c.0, c.1))
            .collect::<Vec<String>>()
            .join("; ");

        let csrf_token = cookie_jar
            .get("twitter.com", "/", "ct0")
            .map(|cookie| cookie.value().to_string());

        (cookie_header, csrf_token)
    };

    if let Some(value) = csrf_token {
        headers.insert("X-Csrf-Token", HeaderValue::from_str(&value).unwrap());
        headers.insert(COOKIE, HeaderValue::from_str(&cookie_header)?);

        tracing::info!("Building Should follow");
        let mut should_follow: Vec<String> = vec![];
        let mut seen_tweets: Vec<String> = vec![];
        let mut user_info_map: HashMap<String, UserInfo> = HashMap::new();
        for u in users {
            let id = get_user_id_by_screen_name(&client, &mut headers, u.to_string()).await?;
            should_follow.push(id.clone());
            user_info_map.insert(
                id,
                UserInfo {
                    username: u.to_string(),
                    last_tweet: "".into(),
                    status: "Waiting for Tweet".into(),
                },
            );
        }
        tx.send(user_info_map.values().cloned().collect()).unwrap();
        tracing::info!("Building Should follow DONE");

        let id = get_user_id_by_screen_name(&client, &mut headers, "lytraPoste52105".to_string())
            .await?;
        let following = get_following(&client, &mut headers, id).await?;
        let _ = unfollow_users(&client, &mut headers, following).await?;

        sleep(Duration::from_secs(2)).await;

        let _ = follow_users(&client, &mut headers, should_follow).await?;

        log::info!(target:"app", "Twitter monitor initialized and ready!");

        let mut first = true;
        loop {
            let start = Instant::now();
            match fetch_latest(&client, &mut headers, &mut seen_tweets).await {
                Some(tweets) => {
                    for msg in tweets.iter() {
                        if !first {
                            let status =
                                tasks::start_user_tasks(msg.1.clone(), state.clone()).await?;
                            if let Some(user) = user_info_map.get_mut(&msg.0) {
                                user.last_tweet = msg.1.clone();
                                user.status = status;
                            }
                            tx.send(user_info_map.values().cloned().collect()).unwrap();
                        }
                    }
                    first = false;
                }
                None => {
                    eprintln!("Error fetching tweet");
                }
            }
            let elapsed = start.elapsed();
            if elapsed < Duration::from_secs(2) {
                sleep(Duration::from_secs(2) - elapsed).await;
            }
        }
    }

    Ok(())
}

pub async fn fetch_latest(
    client: &Client,
    headers: &mut HeaderMap,
    seen_tweets: &mut Vec<String>,
) -> Option<Vec<(String, String)>> {
    match get_latest_timeline(&client, headers, seen_tweets).await {
        Ok(tweets) => {
            let new = check_if_new_tweet(tweets, seen_tweets);
            return Some(new);
        }
        Err(_) => {
            return None;
        }
    }
}

pub fn check_if_new_tweet(
    tweets: HashMap<String, Vec<(String, String)>>,
    seen_tweets: &mut Vec<String>,
) -> Vec<(String, String)> {
    let mut res = vec![];
    for (user, tweets_map) in &tweets {
        for tweet in tweets_map.iter() {
            if !seen_tweets.contains(&tweet.1) {
                tracing::info!("New Tweet: {}", tweet.0);
                res.push((user.to_string(), tweet.0.to_string()));
                seen_tweets.push(tweet.1.clone());
            }
        }
    }

    res
}

pub async fn get_latest_timeline(
    _client: &Client,
    headers: &mut HeaderMap,
    seen_tweets: &mut Vec<String>,
) -> Result<HashMap<String, Vec<(String, String)>>, Box<dyn Error>> {
    let mut tweets: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let params = json!({
        "variables": {
            "count": 20,
            "includePromotedContent": true,
            "latestControlAvailable": true,
            "requestContext": "launch",
            "withCommunity": true,
            "seenTweetIds": seen_tweets.clone(),
        },
        "queryId": "U0cdisy7QFIoTfu3-Okw0A",
        "features": {
            "creator_subscriptions_tweet_preview_api_enabled": true,
            "c9s_tweet_anatomy_moderator_badge_enabled": true,
            "tweetypie_unmention_optimization_enabled": true,
            "responsive_web_edit_tweet_api_enabled": true,
            "graphql_is_translatable_rweb_tweet_is_translatable_enabled": true,
            "view_counts_everywhere_api_enabled": true,
            "longform_notetweets_consumption_enabled": true,
            "responsive_web_twitter_article_tweet_consumption_enabled": true,
            "tweet_awards_web_tipping_enabled": false,
            "longform_notetweets_rich_text_read_enabled": true,
            "longform_notetweets_inline_media_enabled": true,
            "rweb_video_timestamps_enabled": true,
            "responsive_web_graphql_exclude_directive_enabled": true,
            "verified_phone_label_enabled": false,
            "freedom_of_speech_not_reach_fetch_enabled": true,
            "standardized_nudges_misinfo": true,
            "tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled": true,
            "responsive_web_media_download_video_enabled": false,
            "responsive_web_graphql_skip_user_profile_image_extensions_enabled": false,
            "responsive_web_graphql_timeline_navigation_enabled": true,
            "responsive_web_enhance_cards_enabled": false
        },
    });

    let url = "https://twitter.com/i/api/graphql/U0cdisy7QFIoTfu3-Okw0A/HomeLatestTimeline";

    let req = Client::builder()
        .build()?
        .post(url)
        .headers(headers.clone())
        .json(&params);
    let res = req.send().await?;

    let text: Value = res.json().await?;
    let instructions = text["data"]["home"]["home_timeline_urt"]["instructions"]
        .as_array()
        .unwrap();
    for instruction in instructions.iter() {
        if instruction["entries"].is_array() {
            for entry in instruction["entries"].as_array().unwrap().iter() {
                if entry["entryId"].as_str().unwrap().starts_with("tweet") {
                    let id = entry["content"]["itemContent"]["tweet_results"]["result"]["legacy"]
                        ["user_id_str"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    let tweet_id = entry["content"]["itemContent"]["tweet_results"]["result"]
                        ["legacy"]["id_str"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    let tweet = entry["content"]["itemContent"]["tweet_results"]["result"]
                        ["legacy"]["full_text"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    tweets
                        .entry(id)
                        .or_insert_with(Vec::new)
                        .push((tweet, tweet_id));
                }
            }
        }
    }

    Ok(tweets.clone())
}

#[allow(dead_code)]
pub async fn get_latest_tweet(
    _client: &Client,
    headers: &mut HeaderMap,
) -> Result<String, Box<dyn Error>> {
    let url = "https://twitter.com/i/api/graphql/vMkJyzx1wdmvOeeNG0n6Wg/UserTweetsAndReplies?variables=%7B%22userId%22%3A%20%221790897799313154048%22%2C%20%22count%22%3A%201%2C%20%22includePromotedContent%22%3A%20true%2C%20%22withQuickPromoteEligibilityTweetFields%22%3A%20true%2C%20%22withVoice%22%3A%20true%2C%20%22withV2Timeline%22%3A%20true%7D&features=%7B%22creator_subscriptions_tweet_preview_api_enabled%22%3A%20true%2C%20%22c9s_tweet_anatomy_moderator_badge_enabled%22%3A%20true%2C%20%22tweetypie_unmention_optimization_enabled%22%3A%20true%2C%20%22responsive_web_edit_tweet_api_enabled%22%3A%20true%2C%20%22graphql_is_translatable_rweb_tweet_is_translatable_enabled%22%3A%20true%2C%20%22view_counts_everywhere_api_enabled%22%3A%20true%2C%20%22longform_notetweets_consumption_enabled%22%3A%20true%2C%20%22responsive_web_twitter_article_tweet_consumption_enabled%22%3A%20true%2C%20%22tweet_awards_web_tipping_enabled%22%3A%20false%2C%20%22longform_notetweets_rich_text_read_enabled%22%3A%20true%2C%20%22longform_notetweets_inline_media_enabled%22%3A%20true%2C%20%22rweb_video_timestamps_enabled%22%3A%20true%2C%20%22responsive_web_graphql_exclude_directive_enabled%22%3A%20true%2C%20%22verified_phone_label_enabled%22%3A%20false%2C%20%22freedom_of_speech_not_reach_fetch_enabled%22%3A%20true%2C%20%22standardized_nudges_misinfo%22%3A%20true%2C%20%22tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled%22%3A%20true%2C%20%22responsive_web_media_download_video_enabled%22%3A%20false%2C%20%22responsive_web_graphql_skip_user_profile_image_extensions_enabled%22%3A%20false%2C%20%22responsive_web_graphql_timeline_navigation_enabled%22%3A%20true%2C%20%22responsive_web_enhance_cards_enabled%22%3A%20false%7D";

    // let res = client.get(url).headers(headers.clone()).send().await?;
    let res = Client::builder()
        .build()?
        .get(url)
        .headers(headers.clone())
        .send()
        .await?;
    let text: Value = res.json().await?;
    let tweet = text["data"]["user"]["result"]["timeline_v2"]["timeline"]["instructions"][1]
        ["entries"][0]["content"]["itemContent"]["tweet_results"]["result"]["legacy"]["full_text"]
        .as_str()
        .unwrap()
        .to_string();

    Ok(tweet)
}

pub async fn follow_users(
    _client: &Client,
    headers: &mut HeaderMap,
    users: Vec<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut headers = headers.clone();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );

    let url = "https://twitter.com/i/api/1.1/friendships/create.json";

    for user in users.iter() {
        let mut data = HashMap::new();
        data.insert("include_profile_interstitial_type", "1");
        data.insert("include_blocking", "1");
        data.insert("include_blocked_by", "1");
        data.insert("include_followed_by", "1");
        data.insert("include_want_retweets", "1");
        data.insert("include_mute_edge", "1");
        data.insert("include_can_dm", "1");
        data.insert("include_can_media_tag", "1");
        data.insert("include_ext_is_blue_verified", "1");
        data.insert("include_ext_verified_type", "1");
        data.insert("include_ext_profile_image_shape", "1");
        data.insert("skip_status", "1");
        data.insert("user_id", user);

        let encoded_data = serde_urlencoded::to_string(&data)?;

        // Make the POST request
        let _ = Client::builder()
            .build()?
            .post(url)
            .headers(headers.clone())
            .body(encoded_data)
            .send()
            .await?;
    }

    Ok(())
}

pub async fn get_following(
    _client: &Client,
    headers: &mut HeaderMap,
    id: String,
) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
    let url = format!("https://twitter.com/i/api/graphql/2vUj-_Ek-UmBVDNtd8OnQA/Following?variables=%7B%22userId%22%3A%20%22{id}%22%2C%20%22count%22%3A%2020%2C%20%22includePromotedContent%22%3A%20false%7D&features=%7B%22creator_subscriptions_tweet_preview_api_enabled%22%3A%20true%2C%20%22c9s_tweet_anatomy_moderator_badge_enabled%22%3A%20true%2C%20%22tweetypie_unmention_optimization_enabled%22%3A%20true%2C%20%22responsive_web_edit_tweet_api_enabled%22%3A%20true%2C%20%22graphql_is_translatable_rweb_tweet_is_translatable_enabled%22%3A%20true%2C%20%22view_counts_everywhere_api_enabled%22%3A%20true%2C%20%22longform_notetweets_consumption_enabled%22%3A%20true%2C%20%22responsive_web_twitter_article_tweet_consumption_enabled%22%3A%20true%2C%20%22tweet_awards_web_tipping_enabled%22%3A%20false%2C%20%22longform_notetweets_rich_text_read_enabled%22%3A%20true%2C%20%22longform_notetweets_inline_media_enabled%22%3A%20true%2C%20%22rweb_video_timestamps_enabled%22%3A%20true%2C%20%22responsive_web_graphql_exclude_directive_enabled%22%3A%20true%2C%20%22verified_phone_label_enabled%22%3A%20false%2C%20%22freedom_of_speech_not_reach_fetch_enabled%22%3A%20true%2C%20%22standardized_nudges_misinfo%22%3A%20true%2C%20%22tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled%22%3A%20true%2C%20%22responsive_web_media_download_video_enabled%22%3A%20false%2C%20%22responsive_web_graphql_skip_user_profile_image_extensions_enabled%22%3A%20false%2C%20%22responsive_web_graphql_timeline_navigation_enabled%22%3A%20true%2C%20%22responsive_web_enhance_cards_enabled%22%3A%20false%7D");

    let mut users: Vec<String> = vec![];

    let req = Client::builder().build()?.get(url).headers(headers.clone());
    let res = req.send().await?;
    let text: Value = res.json().await?;
    let instructions = text["data"]["user"]["result"]["timeline"]["timeline"]["instructions"]
        .as_array()
        .unwrap();
    for instruction in instructions.iter() {
        if instruction["entries"].is_array() {
            if instruction["entries"][0]["entryId"]
                .as_str()
                .unwrap()
                .starts_with("user")
            {
                let id = instruction["entries"][0]["content"]["itemContent"]["user_results"]
                    ["result"]["rest_id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                users.push(id);
            }
        }
    }

    Ok(users)
}

pub async fn unfollow_users(
    _client: &Client,
    headers: &mut HeaderMap,
    users: Vec<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut headers = headers.clone();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );

    let url = "https://twitter.com/i/api/1.1/friendships/destroy.json"; // Replace with the actual API endpoint

    for user in users.iter() {
        let mut data = HashMap::new();
        data.insert("include_profile_interstitial_type", "1");
        data.insert("include_blocking", "1");
        data.insert("include_blocked_by", "1");
        data.insert("include_followed_by", "1");
        data.insert("include_want_retweets", "1");
        data.insert("include_mute_edge", "1");
        data.insert("include_can_dm", "1");
        data.insert("include_can_media_tag", "1");
        data.insert("include_ext_is_blue_verified", "1");
        data.insert("include_ext_verified_type", "1");
        data.insert("include_ext_profile_image_shape", "1");
        data.insert("skip_status", "1");
        data.insert("user_id", user);

        let encoded_data = serde_urlencoded::to_string(&data)?;

        // Make the POST request
        let _ = Client::builder()
            .build()?
            .post(url)
            .headers(headers.clone())
            .body(encoded_data)
            .send()
            .await?;
    }

    Ok(())
}

pub async fn get_user_id_by_screen_name(
    _client: &Client,
    headers: &mut HeaderMap,
    name: String,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let url = format!("https://twitter.com/i/api/graphql/NimuplG1OB7Fd2btCLdBOw/UserByScreenName?variables=%7B%22screen_name%22%3A%20%22{name}%22%2C%20%22withSafetyModeUserFields%22%3A%20false%7D&features=%7B%22hidden_profile_likes_enabled%22%3A%20true%2C%20%22hidden_profile_subscriptions_enabled%22%3A%20true%2C%20%22responsive_web_graphql_exclude_directive_enabled%22%3A%20true%2C%20%22verified_phone_label_enabled%22%3A%20false%2C%20%22subscriptions_verification_info_is_identity_verified_enabled%22%3A%20true%2C%20%22subscriptions_verification_info_verified_since_enabled%22%3A%20true%2C%20%22highlights_tweets_tab_ui_enabled%22%3A%20true%2C%20%22responsive_web_twitter_article_notes_tab_enabled%22%3A%20false%2C%20%22creator_subscriptions_tweet_preview_api_enabled%22%3A%20true%2C%20%22responsive_web_graphql_skip_user_profile_image_extensions_enabled%22%3A%20false%2C%20%22responsive_web_graphql_timeline_navigation_enabled%22%3A%20true%7D&fieldToggles=%7B%22withAuxiliaryUserLabels%22%3A%20false%7D");

    let req = Client::builder().build()?.get(url).headers(headers.clone());
    let res = req.send().await?;
    let text: Value = res.json().await?;
    let id = text["data"]["user"]["result"]["rest_id"]
        .as_str()
        .unwrap()
        .to_string();

    Ok(id)
}

pub async fn login(
    client: &Client,
    headers: &mut HeaderMap,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // First request to get guest token
    let guest_token = get_guest_token(&client, &headers).await?;

    // Update headers with guest token
    headers.insert("x-guest-token", HeaderValue::from_str(&guest_token)?);

    // Login flow steps
    let flow_token = initiate_login_flow(&client, &headers).await?;
    let username = "ztnaohzd";
    let _email = "ztnaohzd@sharklasers.com";
    let password = "dL2FhRW8u9Mx"; // Replace with actual password

    let (flow_token, task_id) = submit_username(&client, &headers, &flow_token, username).await?;

    if task_id == "LoginEnterAlternateIdentifierSubtask" {
        return Err("Not implemented yet".into());
    }

    let flow_token = submit_password(&client, &headers, &flow_token, password).await?;
    complete_login(&client, &headers, &flow_token).await?;

    Ok(())
}

// Helper function to get guest token
async fn get_guest_token(
    client: &Client,
    headers: &HeaderMap,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let res = client
        .post("https://api.twitter.com/1.1/guest/activate.json")
        .headers(headers.clone())
        .send()
        .await?;
    let res: Value = res.json().await?;
    Ok(res["guest_token"].as_str().unwrap().to_string())
}

// Helper function to initiate login flow
async fn initiate_login_flow(
    client: &Client,
    headers: &HeaderMap,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let res = client
        .post("https://api.twitter.com/1.1/onboarding/task.json?flow_name=login")
        .headers(headers.clone())
        .send()
        .await?;
    let res: Value = res.json().await?;
    let flow_token: String = res["flow_token"].as_str().unwrap().to_string();

    let res = client
        .post("https://api.twitter.com/1.1/onboarding/task.json")
        .headers(headers.clone())
        .json(&json!({ "flow_token": flow_token }))
        .send()
        .await?;
    let res: Value = res.json().await?;
    Ok(res["flow_token"].as_str().unwrap().to_string())
}

// Helper function to submit username
async fn submit_username(
    client: &Client,
    headers: &HeaderMap,
    flow_token: &str,
    username: &str,
) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
    let data = json!({
        "flow_token": flow_token,
        "subtask_inputs": [
             {
                "subtask_id": "LoginEnterUserIdentifierSSO",
                "settings_list": {
                    "setting_responses": [
                        {
                            "key": "user_identifier",
                            "response_data": {
                                "text_data": {
                                    "result": username
                                }
                            }
                        }
                    ],
                    "link": "next_link"
                }
            }
        ]
    });

    let res = client
        .post("https://api.twitter.com/1.1/onboarding/task.json")
        .headers(headers.clone())
        .json(&data)
        .send()
        .await?;
    let res: Value = res.json().await?;
    Ok((
        res["flow_token"].as_str().unwrap().to_string(),
        res["subtasks"][0]["subtask_id"]
            .as_str()
            .unwrap()
            .to_string(),
    ))
}

// Helper function to submit password
async fn submit_password(
    client: &Client,
    headers: &HeaderMap,
    flow_token: &str,
    password: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let data = json!({
        "flow_token": flow_token,
        "subtask_inputs": [
             {
                "subtask_id": "LoginEnterPassword",
                "enter_password": {
                    "password": password,
                    "link": "next_link"
                }
            }
        ]
    });

    let res = client
        .post("https://api.twitter.com/1.1/onboarding/task.json")
        .headers(headers.clone())
        .json(&data)
        .send()
        .await?;
    let res: Value = res.json().await?;
    Ok(res["flow_token"].as_str().unwrap().to_string())
}

// Helper function to complete login
async fn complete_login(
    client: &Client,
    headers: &HeaderMap,
    flow_token: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let data = json!({
        "flow_token": flow_token,
        "subtask_inputs": [
             {
                "subtask_id": "AccountDuplicationCheck",
                "check_logged_in_account": {
                    "link": "AccountDuplicationCheck_false"
                }
            }
        ]
    });

    let _res = client
        .post("https://api.twitter.com/1.1/onboarding/task.json")
        .headers(headers.clone())
        .json(&data)
        .send()
        .await?;

    Ok(())
}

pub fn get_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(REFERER, HeaderValue::from_static("https://twitter.com/"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
        ),
    );
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en-US,en;q=0.5"),
    );
    headers.insert("x-twitter-client-language", HeaderValue::from_static("en"));

    headers
}
