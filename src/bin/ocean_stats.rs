use reqwest;
use serde::Deserialize;
use anyhow::{Result, Context};
use serde_json::Value;
use std::env;

// Structs for partial deserialization if needed, otherwise use Value for flexibility
#[derive(Debug, Deserialize)]
struct PoolDetails {
    // Assuming some fields, but using Value for the rest
    #[serde(flatten)]
    data: Value,
}

#[derive(Debug, Deserialize)]
struct Block {
    height: u64,
    // We'll just take the height for display, rest can be Value if not used
    #[serde(flatten)]
    data: Value,
}

async fn fetch_ocean_data_rust() -> Result<()> {
    let base_url = "https://mempool.space/api/v1/mining";
    let slug = "ocean";

    let endpoints = [
        ("Pool Details", format!("{}/pool/{}", base_url, slug)),
        ("Hashrate History", format!("{}/pool/{}/hashrate", base_url, slug)),
        ("Recent Blocks", format!("{}/pool/{}/blocks", base_url, slug)),
    ];

    println!("--- Querying mempool.space for Pool: {} ---", slug.to_uppercase());

    for (title, url) in endpoints.into_iter() {
        match reqwest::get(&url).await {
            Ok(response) => {
                response.raise_for_status().context(format!("HTTP error for {}", title))?;
                let data: Value = response.json().await?;

                println!("
[+] {}:", title);
                if data.is_array() {
                    // Print first 2 items if it's a list
                    if let Some(arr) = data.as_array() {
                        let preview: Vec<&Value> = arr.iter().take(2).collect();
                        println!("{}", serde_json::to_string_pretty(&preview)?);
                        println!("... ({} total items returned)", arr.len());
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(&data)?);
                }
            },
            Err(e) => {
                eprintln!("[-] Error fetching {}: {}", title, e);
            }
        }
    }

    Ok(())
}

fn generate_ocean_config_env_rust() {
    env::set_var("POOL_URL", "mine.ocean.xyz:3334");
    env::set_var("POOL_API_SLUG", "ocean");

    println!("
--- Local Environment Configured ---");
    println!("POOL_URL: {}", env::var("POOL_URL").unwrap_or_else(|_| "N/A".to_string()));
    println!("API_SLUG: {}", env::var("POOL_API_SLUG").unwrap_or_else(|_| "N/A".to_string()));
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Configure the local environment
    generate_ocean_config_env_rust();

    // 2. Fetch live data from the API
    fetch_ocean_data_rust().await
}
