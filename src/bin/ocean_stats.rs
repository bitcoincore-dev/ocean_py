use std::env;

use anyhow::Result;
use ocean_loss_estimator_rs::utils::fetch_from_mirror;
use serde::Deserialize; // Retain for PoolDetails and Block
use serde_json::Value;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct PoolDetails {
    #[serde(flatten)]
    data: Value,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Block {
    height: u64,
    #[serde(flatten)]
    data: Value,
}

async fn fetch_ocean_data_rust() -> Result<()> {
    let base_path = "/api/v1/mining";
    let slug = "ocean";

    let endpoints = [
        ("Pool Details", format!("{}/pool/{}", base_path, slug)),
        (
            "Hashrate History",
            format!("{}/pool/{}/hashrate", base_path, slug),
        ),
        (
            "Recent Blocks",
            format!("{}/pool/{}/blocks", base_path, slug),
        ),
    ];

    println!(
        "--- Querying mempool.space for Pool: {} ---",
        slug.to_uppercase()
    );

    for (title, path) in endpoints.into_iter() {
        match fetch_from_mirror(&path, 0, 10).await {
            Ok(data) => {
                println!(
                    "
[+] {}:",
                    title
                );
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
            }
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

    println!(
        "
--- Local Environment Configured ---"
    );
    println!(
        "POOL_URL: {}",
        env::var("POOL_URL").unwrap_or_else(|_| "N/A".to_string())
    );
    println!(
        "API_SLUG: {}",
        env::var("POOL_API_SLUG").unwrap_or_else(|_| "N/A".to_string())
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Configure the local environment
    generate_ocean_config_env_rust();

    // 2. Fetch live data from the API
    fetch_ocean_data_rust().await
}
