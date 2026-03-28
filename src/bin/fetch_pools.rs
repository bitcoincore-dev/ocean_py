use reqwest;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Pool {
    id: String,
    name: String,
    slug: String,
    #[serde(flatten)] // Capture other fields dynamically
    extra: serde_json::Value,
}

async fn fetch_and_save_pool_data() -> Result<()> {
    // Primary URL (from Python script, seems to be sweetsats.io first, then mempool.space as fallback)
    let primary_url = "https://mempool.space/api/v1/mining/pools/1y"; // Corrected to mempool.space for consistency with actual usage, Python's var name was misleading
    let failover_url = "https://mempool.space/api/v1/mining/pools/1y"; // Actually mempool.space is fallback in Python, will use this as a reference if primary fails.

    let output_file = "pools-1y.json";

    println!("--- Fetching Pool Data (1Y) ---"); // Python script had 3Y, but URL is 1Y
    
    let response_data: Vec<Pool>;

    // Attempt to fetch from primary_url
    match reqwest::get(primary_url).await {
        Ok(response) => {
            if response.status().is_success() {
                response_data = response.json::<Vec<Pool>>().await?;
                println!("[+] Successfully fetched from {}.", primary_url);
            } else {
                println!("[-] {} returned {}. Trying failover URL...", primary_url, response.status());
                let failover_response = reqwest::get(failover_url).await?;
                response_data = failover_response.error_for_status().context(format!("HTTP error for failover URL {}", failover_url))?.json::<Vec<Pool>>().await?;
                println!("[+] Successfully fetched from {}.", failover_url);
            }
        },
        Err(e) => {
            println!("[-] Error fetching from {}: {}. Trying failover URL...", primary_url, e);
            let failover_response = reqwest::get(failover_url).await?;
            response_data = failover_response.error_for_status().context(format!("HTTP error for failover URL {}", failover_url))?.json::<Vec<Pool>>().await?;
            println!("[+] Successfully fetched from {}.", failover_url);
        }
    }

    // Write to JSON file
    let json_string = serde_json::to_string_pretty(&response_data)?;
    tokio::fs::File::create(output_file).await?.write_all(json_string.as_bytes()).await?;

    println!("[+] Successfully wrote {} pool entries to {}", response_data.len(), output_file);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    fetch_and_save_pool_data().await
}
