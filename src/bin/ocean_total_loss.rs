use reqwest;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tokio::time::{sleep, Duration};
use tokio::io::AsyncWriteExt; // Re-added this import
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;
use dashmap::DashMap;
use serde_json::Value;

const MIRRORS: &[&str] = &[
    "https://mempool.space",
    "https://mempool.sweetsats.io"
];

#[derive(Debug, Deserialize, Serialize, Clone)]
struct BlockExtras {
    #[serde(rename = "matchRate")]
    match_rate: Option<f64>,
    reward: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Block {
    height: u64,
    timestamp: u64,
    extras: Option<BlockExtras>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PriceData {
    #[serde(rename = "USD")]
    usd: Option<f64>,
}

#[derive(Debug, Serialize, Clone)]
struct ProcessedBlockOutput {
    height: u64,
    match_rate: f64,
    loss_usd: f64,
}

async fn fetch_with_failover(path: &str, timeout_secs: u64) -> Result<(Value, String)> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(timeout_secs)).build()?;

    for base_url in MIRRORS {
        let url = format!("{}{}", base_url, path);
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let data = response.json().await?;
                    return Ok((data, base_url.to_string()));
                }
                // If status is not 200, but not an error that breaks the chain (e.g., 429), try next mirror
                if response.status().as_u16() == 429 { // Too many requests
                    continue;
                }
            },
            Err(_) => {},
        }
    }
    Err(anyhow::anyhow!("Failed to fetch from all mirrors for path: {}", path))
}

async fn get_pool_stats_rust() -> Result<u64> {
    let (data, _) = fetch_with_failover("/api/v1/mining/pool/ocean", 10).await?;
    let block_count = data.get("pool_stats")
                          .and_then(|ps| ps.get("blockCount"))
                          .and_then(|bc| bc.as_u64())
                          .unwrap_or(832);
    Ok(block_count)
}

async fn fetch_full_ocean_report_rust() -> Result<()> {
    let slug = "ocean";
    let mut all_blocks: Vec<Block> = Vec::new();
    let mut last_height: Option<u64> = None;

    let total_expected_blocks = get_pool_stats_rust().await?;

    println!("--- OCEAN History Audit ---");
    println!("Total Blocks Expected: {}", total_expected_blocks);

    // Initialize Progress Bar for crawling blocks
    let pb_crawl = ProgressBar::new(total_expected_blocks);
    pb_crawl.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}").unwrap()
        .progress_chars("#>- "));
    pb_crawl.set_message("Crawling Blocks");

    while pb_crawl.position() < total_expected_blocks {
        let path = match last_height {
            Some(h) => format!("/api/v1/mining/pool/{}/blocks/{}", slug, h),
            None => format!("/api/v1/mining/pool/{}/blocks", slug),
        };

        let (batch_val, _) = fetch_with_failover(&path, 10).await?;
        let batch: Vec<Block> = serde_json::from_value(batch_val)?;

        if batch.is_empty() {
            pb_crawl.set_message("Done: Reached the end of the block chain.");
            break;
        }

        all_blocks.extend(batch.into_iter());
        last_height = Some(all_blocks.last().unwrap().height);

        pb_crawl.set_position(all_blocks.len() as u64);
        sleep(Duration::from_millis(300)).await; // Python uses 0.3s sleep
    }
    pb_crawl.finish_with_message("Block crawling complete.");

    // Processing with Historical Prices
    let price_cache: Arc<DashMap<i64, f64>> = Arc::new(DashMap::new());
    let mut join_set: tokio::task::JoinSet<Result<ProcessedBlockOutput, anyhow::Error>> = tokio::task::JoinSet::new();
    let mut processed_data: Vec<ProcessedBlockOutput> = Vec::new();
    let mut total_loss_usd = 0.0;

    println!("
{:<10} | {:<10} | {:<10}", "Height", "Match Rate", "Loss (USD)");
    println!("{:->40}", "");

    let pb_process = ProgressBar::new(all_blocks.len() as u64);
    pb_process.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}").unwrap()
        .progress_chars("#>- "));
    pb_process.set_message("Calculating Loss (USD)");

    for (i, block) in all_blocks.into_iter().enumerate() {
        let cache_clone = price_cache.clone();
        join_set.spawn(async move {
            let timestamp = block.timestamp as i64;
            let extras = block.extras.unwrap_or(BlockExtras { match_rate: Some(0.0), reward: Some(0) });

            let match_rate = extras.match_rate.unwrap_or(100.0);
            let actual_reward = extras.reward.unwrap_or(0);

            let expected_reward = if match_rate > 0.0 && match_rate < 100.0 {
                ((actual_reward as f64 * 100.0) / match_rate) as u64
            } else {
                actual_reward
            };
            let loss_sats = expected_reward.saturating_sub(actual_reward);

            let mut hist_price = 0.0;
            if let Some(price) = cache_clone.get(&timestamp) {
                hist_price = *price;
            } else {
                // Fetch price if not in cache
                let price_path = format!("/api/v1/historical-price?timestamp={}&currency=USD", timestamp);
                if let Ok((price_data_val, _)) = fetch_with_failover(&price_path, 5).await {
                    if let Some(usd_price) = price_data_val.get("usd").and_then(|u| u.as_f64()) {
                        hist_price = usd_price;
                        cache_clone.insert(timestamp, usd_price);
                    } else {
                         // Python uses a default of 74000.0 if price_data is None/empty, so we do too
                        hist_price = 74000.0;
                    }
                } else { // Handle fetch_with_failover error for price
                    hist_price = 74000.0;
                }
            }

            let loss_usd = (loss_sats as f64 / 100_000_000.0) * hist_price;

            Ok(ProcessedBlockOutput {
                height: block.height,
                match_rate: (match_rate * 100.0).round() / 100.0, // Python rounds to 2 decimal places
                loss_usd: (loss_usd * 100.0).round() / 100.0,
            })
        });
    }

    while let Some(res) = join_set.join_next().await {
        match res? {
            Ok(output) => {
                println!("{:<10} | {:<10.2} | {:<10.2}", output.height, output.match_rate, output.loss_usd);
                total_loss_usd += output.loss_usd;
            },
            Err(e) => eprintln!("Error processing block: {}", e),
        }
        pb_process.inc(1);
    }
    pb_process.finish_with_message("Loss calculation complete.");

    println!("{:->40}", "");
    println!("TOTAL BLOCKS: {}", processed_data.len());
    println!("TOTAL LOSS:   ${:.2}", (total_loss_usd * 100.0).round() / 100.0);

    // Save to file
    let output_file = "ocean_historical_report.json";
    let json_string = serde_json::to_string_pretty(&processed_data)?;
    let mut file = tokio::fs::File::create(output_file).await?;
    file.write_all(json_string.as_bytes()).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    fetch_full_ocean_report_rust().await
}
