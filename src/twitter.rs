use std::{collections::HashMap, error::Error, fs, str::FromStr, sync::Arc};

use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, REFERER, USER_AGENT},
    Client, Url,
};
use reqwest_cookie_store::CookieStoreMutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize)]
struct SavedCookie {
    name: String,
    value: String,
    domain: String,
    path: String,
}

#[derive(Serialize, Deserialize)]
struct SavedCookies(Vec<SavedCookie>);

pub async fn login() -> Result<(), Box<dyn Error>> {
    // Initialize cookie store and HTTP client
    let cookie_store = Arc::new(CookieStoreMutex::default());
    let client = Client::builder()
        .cookie_store(true)
        .cookie_provider(cookie_store.clone())
        .build()?;

    // Set common headers
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

    // First request to get guest token
    let guest_token = get_guest_token(&client, &headers).await?;

    // Update headers with guest token
    headers.insert("x-guest-token", HeaderValue::from_str(&guest_token)?);

    // Login flow steps
    let flow_token = initiate_login_flow(&client, &headers).await?;
    let username = "hopecorecrypto"; // Replace with actual username
    let password = "Pranjic2003"; // Replace with actual password

    let (flow_token, task_id) = submit_username(&client, &headers, &flow_token, username).await?;

    if task_id == "LoginEnterAlternateIdentifierSubtask" {
        return Err("Not implemented yet".into());
    }

    let flow_token = submit_password(&client, &headers, &flow_token, password).await?;
    complete_login(&client, &headers, &flow_token).await?;

    let cookie_jar = cookie_store.lock().unwrap();
    for c in cookie_jar.iter_any() {
        println!("{c:#?}");
    }
    if let Some(cookie) = cookie_jar.get("twitter.com", "/", "ct0") {
        let value = cookie.value().to_string();
        let _ = get_latest_tweet(&client, value, &mut headers).await;
    }

    Ok(())
}

// Helper function to get guest token
async fn get_guest_token(client: &Client, headers: &HeaderMap) -> Result<String, Box<dyn Error>> {
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
) -> Result<String, Box<dyn Error>> {
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
) -> Result<(String, String), Box<dyn Error>> {
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
) -> Result<String, Box<dyn Error>> {
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
) -> Result<(), Box<dyn Error>> {
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

    let res = client
        .post("https://api.twitter.com/1.1/onboarding/task.json")
        .headers(headers.clone())
        .json(&data)
        .send()
        .await?;
    Ok(())
}

pub async fn get_latest_tweet(
    client: &Client,
    csrf_token: String,
    headers: &mut HeaderMap,
) -> Result<(), Box<dyn Error>> {
    headers.insert(
        "X-Twitter-Auth-Type",
        HeaderValue::from_static("OAuth2Session"),
    );
    headers.insert("X-Twitter-Active-User", HeaderValue::from_static("yes"));
    headers.insert("X-Csrf-Token", HeaderValue::from_str(&csrf_token).unwrap());

    let variables = json!({
        "userId": "1798648988700389376",
        "count": 1,
        "includePromotedContent": true,
        "withQuickPromoteEligibilityTweetFields": true,
        "withVoice": true,
        "withV2Timeline": true,
    });

    let features = json!({
        "creator_subscriptions_quote_tweet_preview_enabled": true,
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
    });

    let params = json!({
        "variables": variables,
        "features": features,
    });

    let user_id = "1798648988700389376"; // You can change this value
    let count = 1;

    let variables = format!(
        r#"{{"userId":"{}","count":{},"includePromotedContent":true,"withQuickPromoteEligibilityTweetFields":true,"withVoice":true,"withV2Timeline":true}}"#,
        user_id, count
    );

    let features = r#"{"creator_subscriptions_quote_tweet_preview_enabled":true,"c9s_tweet_anatomy_moderator_badge_enabled":true,"tweetypie_unmention_optimization_enabled":true,"responsive_web_edit_tweet_api_enabled":true,"graphql_is_translatable_rweb_tweet_is_translatable_enabled":true,"view_counts_everywhere_api_enabled":true,"longform_notetweets_consumption_enabled":true,"responsive_web_twitter_article_tweet_consumption_enabled":true,"tweet_awards_web_tipping_enabled":false,"longform_notetweets_rich_text_read_enabled":true,"longform_notetweets_inline_media_enabled":true,"rweb_video_timestamps_enabled":true,"responsive_web_graphql_exclude_directive_enabled":true,"verified_phone_label_enabled":false,"freedom_of_speech_not_reach_fetch_enabled":true,"standardized_nudges_misinfo":true,"tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled":true,"responsive_web_media_download_video_enabled":false,"responsive_web_graphql_skip_user_profile_image_extensions_enabled":false,"responsive_web_graphql_timeline_navigation_enabled":true,"responsive_web_enhance_cards_enabled":false}"#;

    let params = format!(r#"variables={}&features={}"#, variables, features);

    println!("Params: {}", params);

    let url = format!("https://x.com/i/api/graphql/V7H0Ap3_Hh2FyS75OCDO3Q/UserTweets?{params}");

    // Flatten JSON structure to key-value pairs
    // let mut query_params = HashMap::new();
    // flatten_json("", &params, &mut query_params);

    let url = "https://twitter.com/i/api/graphql/vMkJyzx1wdmvOeeNG0n6Wg/UserTweetsAndReplies?variables=%7B%22userId%22%3A%20%221790897799313154048%22%2C%20%22count%22%3A%2040%2C%20%22includePromotedContent%22%3A%20true%2C%20%22withQuickPromoteEligibilityTweetFields%22%3A%20true%2C%20%22withVoice%22%3A%20true%2C%20%22withV2Timeline%22%3A%20true%7D&features=%7B%22creator_subscriptions_tweet_preview_api_enabled%22%3A%20true%2C%20%22c9s_tweet_anatomy_moderator_badge_enabled%22%3A%20true%2C%20%22tweetypie_unmention_optimization_enabled%22%3A%20true%2C%20%22responsive_web_edit_tweet_api_enabled%22%3A%20true%2C%20%22graphql_is_translatable_rweb_tweet_is_translatable_enabled%22%3A%20true%2C%20%22view_counts_everywhere_api_enabled%22%3A%20true%2C%20%22longform_notetweets_consumption_enabled%22%3A%20true%2C%20%22responsive_web_twitter_article_tweet_consumption_enabled%22%3A%20true%2C%20%22tweet_awards_web_tipping_enabled%22%3A%20false%2C%20%22longform_notetweets_rich_text_read_enabled%22%3A%20true%2C%20%22longform_notetweets_inline_media_enabled%22%3A%20true%2C%20%22rweb_video_timestamps_enabled%22%3A%20true%2C%20%22responsive_web_graphql_exclude_directive_enabled%22%3A%20true%2C%20%22verified_phone_label_enabled%22%3A%20false%2C%20%22freedom_of_speech_not_reach_fetch_enabled%22%3A%20true%2C%20%22standardized_nudges_misinfo%22%3A%20true%2C%20%22tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled%22%3A%20true%2C%20%22responsive_web_media_download_video_enabled%22%3A%20false%2C%20%22responsive_web_graphql_skip_user_profile_image_extensions_enabled%22%3A%20false%2C%20%22responsive_web_graphql_timeline_navigation_enabled%22%3A%20true%2C%20%22responsive_web_enhance_cards_enabled%22%3A%20false%7D";

    headers.remove("Accept-Language");
    headers.remove("x-twitter-client-language");
    headers.remove("x-guest-token");
    headers.remove(AUTHORIZATION);
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA"));
    println!("{:#?}", headers);
    println!("{url}");

    let res = client.get(url).headers(headers.clone()).send().await?;
    println!("{:#?}", res);
    let text: Value = res.json().await?;
    println!("{:#?}", text);

    Ok(())
}

// Function to flatten JSON structure into key-value pairs
fn flatten_json(prefix: &str, value: &Value, map: &mut HashMap<String, String>) {
    match value {
        Value::Object(obj) => {
            for (k, v) in obj {
                let new_prefix = if prefix.is_empty() {
                    k.to_string()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_json(&new_prefix, v, map);
            }
        }
        _ => {
            map.insert(prefix.to_string(), value.to_string());
        }
    }
}

pub async fn monitor() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://x.com/i/api/graphql/V7H0Ap3_Hh2FyS75OCDO3Q/UserTweets";
    let url2 = "https://twitter.com/i/api/graphql/QWF3SzpHmykQHsQMixG0cg/UserTweets";

    let variables = json!({
        "userId": "1798648988700389376",
        "count": 3,
        "includePromotedContent": true,
        "withQuickPromoteEligibilityTweetFields": true,
        "withVoice": true,
        "withV2Timeline": true,
    });

    let features = json!({
        "creator_subscriptions_quote_tweet_preview_enabled": true, // could be false
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
        "responsive_web_media_download_video_enabled": false, // not sure
        "responsive_web_graphql_skip_user_profile_image_extensions_enabled": false,
        "responsive_web_graphql_timeline_navigation_enabled": true,
        "responsive_web_enhance_cards_enabled": false

        // "rweb_tipjar_consumption_enabled": true,
        // "creator_subscriptions_tweet_preview_api_enabled": true,
        // "communities_web_enable_tweet_community_results_fetch": true,
        // "articles_preview_enabled": true,
    });

    let field_toggles = json!({
        "withArticlePlainText": false
    });

    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (X11; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0",
        ),
    );
    headers.insert("Accept", HeaderValue::from_static("*/*"));
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en-US,en;q=0.5"),
    );
    headers.insert(
        "Accept-Encoding",
        HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://x.com/JacobB47030"),
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    // headers.insert(
    //     "X-Client-UUID",
    //     HeaderValue::from_static("a8603dfc-a970-48ce-a4b6-cc8c3546e976"),
    // );
    headers.insert(
        "x-twitter-auth-type",
        HeaderValue::from_static("OAuth2Session"),
    );
    headers.insert("x-csrf-token", HeaderValue::from_static("3fcebc3c8f4bc9408871903645a15cdbf792bdf7dbf58e91ff16817637563d40d6add873b1a2e8aef0390e5efb498f6c843d6765c3587f954bb0bb25a7b2dfc6ecc422d8eeaf3e81e7b155e2927157d1"));
    headers.insert("x-twitter-client-language", HeaderValue::from_static("en"));
    headers.insert("x-twitter-active-user", HeaderValue::from_static("yes"));
    //headers.insert("x-client-transaction-id", HeaderValue::from_static("SKf0p9uVkyQJBGB8Q/LahXMMVMRmfnEoqIxZJltVAwWDHbcflm4bU05NektzeKIHlnIfUUpoYOf7zXZHoBFLVNR57AUiSw"));
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA"));
    headers.insert("Cookie", HeaderValue::from_static("guest_id=171699338187615286; night_mode=2; d_prefs=MToxLGNvbnNlbnRfdmVyc2lvbjoyLHRleHRfdmVyc2lvbjoxMDAw; guest_id_ads=v1%3A171699338187615286; guest_id_marketing=v1%3A171699338187615286; personalization_id=\"v1_AQQZbevMDn4RIji5Olf9HQ==\"; g_state={\"i_p\":1717370893597,\"i_l\":1}; kdt=nTfh5MBzOa37kUNU7vVMSz8M4uKkKgPboEflX43Q; twid=u%3D1790897799313154048; ct0=3fcebc3c8f4bc9408871903645a15cdbf792bdf7dbf58e91ff16817637563d40d6add873b1a2e8aef0390e5efb498f6c843d6765c3587f954bb0bb25a7b2dfc6ecc422d8eeaf3e81e7b155e2927157d1; auth_token=2e60327c920eb50887119e773ae449641cd0a98d; twtr_pixel_opt_in=Y; des_opt_in=Y; lang=en"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let body = json!({
        "variables": variables,
        "features": features,
        // "fieldToggles": field_toggles
    });

    let res = client
        .post(url2)
        .headers(headers)
        .json(&body)
        .send()
        .await?;
    println!("{:#?}", res);

    let text: Value = res.json().await?;
    println!("{}", text);

    Ok(())
}
