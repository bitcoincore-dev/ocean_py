use clap::Parser;
use reqwest;

use anyhow::{Result, Context};
use tokio::time::{sleep, Duration};
use tokio::io::AsyncWriteExt;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[clap(author, version, about = "Fetch and analyze OCEAN mining pool data.")]
struct Args {
    /// Number of sample blocks to print.
    #[clap(long, default_value = "1000")]
    depth: usize,
}

use ocean_loss_estimator_rs::models::{Block, HistoricalPriceData, ProcessedBlockData, CoinbaseInfo};
use ocean_loss_estimator_rs::utils::{fetch_full_historical_prices_rust, fetch_block_transactions_rust};


async fn fetch_all_ocean_blocks_rust(depth_limit: usize) -> Result<()> {
    #[allow(unused_assignments)] // Suppress warning for best_diff
    let slug = "ocean";
    let base_blocks_url = format!("https://mempool.space/api/v1/mining/pool/{}/blocks", slug);

    let mut all_blocks: Vec<Block> = Vec::new();
    let mut last_height: Option<u64> = None;

    let price_file_name = "prices.json".to_string(); // Define as String
    let price_lookup_map: HashMap<i64, f64> = match tokio::fs::File::open(&price_file_name).await {
        Ok(_file) => {
            let content = tokio::fs::read_to_string(&price_file_name).await?;
            let historical_data: HistoricalPriceData = serde_json::from_str(&content)?;
            let price_map: HashMap<i64, f64> = historical_data.prices.into_iter().filter_map(|p| p.usd.map(|usd_val| (p.time, usd_val))).collect();
            println!("Loaded {} historical prices from {}", price_map.len(), price_file_name);
            price_map
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("{} not found. Attempting to fetch full historical prices...", price_file_name);
            let price_map = fetch_full_historical_prices_rust().await?;
            println!("Loaded {} historical prices from {} (after fetch)", price_map.len(), price_file_name);
            price_map
        },
        Err(e) => return Err(anyhow::anyhow!("Error opening {}: {}", price_file_name, e)),
    };

    let mut sorted_timestamps: Vec<i64> = price_lookup_map.keys().copied().collect();
    sorted_timestamps.sort_unstable(); // Sort the timestamps once for binary search

    println!("--- Starting Full History Crawl for OCEAN ---");

    loop {
        let url = match last_height {
            Some(h) => format!("{}/{}", base_blocks_url, h),
            None => base_blocks_url.clone(),
        };

        let response = reqwest::get(&url).await?;
        let batch: Vec<Block> = response.error_for_status().context(format!("HTTP error fetching blocks from {}", url))?.json::<Vec<Block>>().await?;

        if batch.is_empty() {
            break;
        }

        all_blocks.extend(batch.into_iter());
        last_height = Some(all_blocks.last().unwrap().height);

        println!("Fetched {} blocks... (Current Height: {})", all_blocks.len(), last_height.unwrap());

        sleep(Duration::from_millis(500)).await; // Python uses 0.5s sleep
    }

    // 2. Process and Calculate
    let mut total_loss_usd = 0.0;
    let mut processed_data: Vec<ProcessedBlockData> = Vec::new();

    println!("
{:<10} | {:<8} | {:<12} | {:<10}", "Height", "Health", "Loss(丰)", "Loss($)");
    println!("{:->50}", "");

    for (i, b) in all_blocks.iter().enumerate() {
        let match_rate = b.extras.as_ref().and_then(|e| e.match_rate).unwrap_or(0.0).round();
        let actual_reward = b.extras.as_ref().and_then(|e| e.reward).unwrap_or(0);

        let expected_reward = if match_rate > 0.0 && match_rate < 100.0 {
            ((actual_reward as f64 * 100.0) / match_rate) as u64
        } else {
            actual_reward
        };
        let loss_sats = expected_reward.saturating_sub(actual_reward);

        let _timestamp = b.timestamp as i64;

        let btc_usd = {
            let price_timestamp = b.timestamp as i64;
            #[allow(unused_assignments)]
            let mut closest_price: Option<f64> = None;

            match sorted_timestamps.binary_search(&price_timestamp) {
                Ok(exact_idx) => {
                    closest_price = sorted_timestamps.get(exact_idx)
                                                     .and_then(|&ts| price_lookup_map.get(&ts).copied());
                }
                Err(insert_idx) => {
                    #[allow(unused_assignments)]
                    let mut best_diff = i64::MAX;
                    let mut best_ts: Option<i64> = None;

                    if insert_idx > 0 {
                        let prev_ts = sorted_timestamps[insert_idx - 1];
                        let diff = (price_timestamp - prev_ts).abs();
                        if diff < best_diff {
                            best_diff = diff;
                            best_ts = Some(prev_ts);
                        }
                    }

                    if let Some(&next_ts) = sorted_timestamps.get(insert_idx) {
                        let diff = (price_timestamp - next_ts).abs();
                        if diff < best_diff {
                            //best_diff = diff;
                            best_ts = Some(next_ts);
                        }
                    }
                    closest_price = best_ts.and_then(|ts| price_lookup_map.get(&ts).copied());
                }
            }
            closest_price.unwrap_or(0.0) // Default to 0.0 if no price found
        };

        let loss_usd = (loss_sats as f64 / 100_000_000.0) * btc_usd;
        total_loss_usd += loss_usd;

        let processed_block = ProcessedBlockData {
            height: b.height,
            health: match_rate,
            loss_sats,
            loss_usd: (loss_usd * 100.0).round() / 100.0,
            btc_usd: (btc_usd * 100.0).round() / 100.0,
        };
        processed_data.push(processed_block.clone());

        // Print a sample of the first few
        if processed_data.len() <= depth_limit {
            println!("{:<10} | {:>6.2}% | {:<12} | ${:>8.2}",
                     processed_block.height, processed_block.health, processed_block.loss_sats, processed_block.loss_usd);
        }

        // Fetch and display coinbase info for the first 5 blocks
        if i < 5 {
            match fetch_block_transactions_rust(&b.id).await {
                Ok(coinbase_info) => {
                    println!("    Block {}: Miner: {}", b.height, coinbase_info.miner_name.unwrap_or_else(|| "Unknown Miner".to_string()));
                    if !coinbase_info.op_return_data.is_empty() {
                        for op_ret in coinbase_info.op_return_data {
                            println!("        OP_RETURN: {}", op_ret);
                        }
                    }
                },
                Err(e) => eprintln!("    Error fetching coinbase info for block {}: {}", b.height, e),
            }
        }
    }

    // 3. Output Summary
    println!("{:->50}", "");
    println!("TOTAL BLOCKS MINED: {}", all_blocks.len());
    println!("TOTAL CUMULATIVE LOSS: ${:.2}", (total_loss_usd * 100.0).round() / 100.0);

    // Save to file
    let output_file = "ocean_full_history.json";
    let json_string = serde_json::to_string_pretty(&processed_data)?;
    tokio::fs::File::create(output_file).await?.write_all(json_string.as_bytes()).await?;
    println!("
Full dataset saved to: {}", output_file);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    fetch_all_ocean_blocks_rust(args.depth).await
}
