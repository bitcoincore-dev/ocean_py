extern crate hex;

// Reusable functions and structs for the ocean_py project.

use std::sync::Arc;
use dashmap::DashMap;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;
use tokio::time::{Duration, sleep};
use serde::{Deserialize, Serialize};
use std::env;
use clap::Parser;
use std::collections::HashMap;
use anyhow::{Context, Result, anyhow};
use reqwest;

const MIRRORS: &[&str] = &["https://mempool.space", "https://mempool.sweetsats.io"];

pub mod models {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct PriceData {
        pub time: i64,
        #[serde(rename = "USD")]
        pub usd: Option<f64>,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct HistoricalPriceData {
        pub prices: Vec<PriceData>,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct BlockExtras {
        #[serde(rename = "matchRate")]
        pub match_rate: Option<f64>,
        pub reward: Option<u64>,
        pub expected_fees: Option<u64>,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct Block {
        pub height: u64,
        pub id: String,
        pub timestamp: u64,
        pub extras: Option<BlockExtras>,
    }

    #[derive(Debug, Serialize, Clone)]
    pub struct ProcessedBlockData {
        pub height: u64,
        pub health: f64,
        pub loss_sats: u64,
        pub loss_usd: f64,
        pub btc_usd: f64,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct PoolData {
        #[serde(rename = "avgBlockHealth")]
        pub avg_block_health: Option<f64>,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct Vout {
        pub value: u64,
        pub scriptpubkey_asm: String,
        pub scriptpubkey_type: String,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct Vin {
        #[serde(rename = "scriptSig")]
        pub script_sig: Option<String>,
        #[serde(rename = "scriptSig_asm")]
        pub script_sig_asm: Option<String>,
        pub sequence: u32,
        pub witness: Option<Vec<String>>,
        // Other fields can be added if needed, but for coinbase, scriptSig is key
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct Transaction {
        pub txid: String,
        pub version: u32,
        pub locktime: u32,
        pub vin: Vec<Vin>,
        pub vout: Vec<Vout>,
        pub size: u32,
        pub weight: u32,
        pub fee: u64,
        pub status: Option<Value>, // Using Value as status can be complex
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct BlockDetails {
        pub id: String,
        pub height: u64,
        pub timestamp: u64,
        pub tx_count: u32,
        pub size: u32,
        pub weight: u32,
        pub version: u32,
        pub merkle_root: String,
        pub nonce: u32,
        pub bits: u32,
        pub difficulty: f64,
        pub parent: String,
        pub previousblockhash: String,
        pub nextblockhash: Option<String>,
        pub coinbase_alpha: String,
        pub witness_commitment: Option<String>,
        pub median_fee: Option<u64>,
        pub fee_range: Option<Vec<u64>>,
        pub reward: Option<u64>,
        pub avg_fee_rate: Option<f64>,
        pub avg_tx_size: Option<f64>,

        // Additional fields from the mempool.space /block/:hash endpoint
        pub utxo_set_change: Option<i64>,
        pub utxo_set_size: Option<u64>,
        pub total_fee: Option<u64>,
        pub n_outputs: Option<u64>,
        pub total_output: Option<u64>,
    }

    #[derive(Debug, Serialize, Clone)]
    pub struct CoinbaseInfo {
        pub miner_name: Option<String>,
        pub op_return_data: Vec<String>,
    }

    #[derive(Debug, Serialize, Clone)]
    pub struct ProcessedBlockOutput {
        pub height: u64,
        pub match_rate: f64,
        pub loss_usd: f64,
        pub price: f64,
    }
}

// structs for ocean_total_loss_ocean_report_rust
#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PriceDataTotalLoss {
    #[serde(rename = "USD")]
    pub usd: Option<f64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProcessedBlockOutputTotalLoss {
    pub height: u64,
    pub match_rate: f64,
    pub loss_usd: f64,
}

pub async fn get_pool_stats_rust_total_loss() -> Result<u64> {
    let data = utils::fetch_from_mirror("/api/v1/mining/pool/ocean", 0, 10).await?;
    let block_count = data
        .get("pool_stats")
        .and_then(|ps| ps.get("blockCount"))
        .and_then(|bc| bc.as_u64())
        .unwrap_or(832);
    Ok(block_count)
}

pub async fn fetch_concurrent_ocean_report_rust() -> Result<()> {
    println!("--- Parallel OCEAN Audit ---");

    let total_expected_blocks = utils::get_pool_stats_rust().await?;
    let mut all_blocks: Vec<models::Block> = Vec::new();
    let mut last_height: Option<u64> = None;

    // Stage 1: Fast Header Crawl (Sequential)
    let pb_fetch = ProgressBar::new(total_expected_blocks);
    pb_fetch.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}").unwrap()
        .progress_chars("#>- "));
    pb_fetch.set_message("Fetching Headers");

    while pb_fetch.position() < total_expected_blocks {
        let path = match last_height {
            Some(h) => format!("/api/v1/mining/pool/ocean/blocks/{}", h),
            None => "/api/v1/mining/pool/ocean/blocks".to_string(),
        };

        let batch: Vec<models::Block> = serde_json::from_value(utils::fetch_from_mirror(&path, 0, 10).await?)?;

        if batch.is_empty() {
            break;
        }
        all_blocks.extend(batch.into_iter());
        last_height = Some(all_blocks.last().unwrap().height);
        pb_fetch.set_position(all_blocks.len() as u64);
        sleep(Duration::from_millis(100)).await;
    }
    pb_fetch.finish_with_message("Headers fetched.");

    // Stage 2: Parallel Analysis
    let price_cache: Arc<DashMap<i64, f64>> = Arc::new(DashMap::new());
    let mut join_set = tokio::task::JoinSet::new();
    let mut processed_data: Vec<models::ProcessedBlockOutput> = Vec::new();
    let mut total_loss_usd = 0.0;

    println!(
        "Analyzing {} blocks using {} mirrors...",
        all_blocks.len(),
        MIRRORS.len()
    );

    let pb_analyze = ProgressBar::new(all_blocks.len() as u64);
    pb_analyze.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}").unwrap()
        .progress_chars("#>- "));
    pb_analyze.set_message("Pricing & Loss");

    for (i, block) in all_blocks.into_iter().enumerate() {
        let cache_clone = price_cache.clone();
        join_set.spawn(async move { utils::process_single_block(block, i, cache_clone).await });
    }

    while let Some(res) = join_set.join_next().await {
        match res? {
            Ok(output) => {
                processed_data.push(output.clone());
                total_loss_usd += output.loss_usd;
            }
            Err(e) => eprintln!("Error processing block: {}", e),
        }
        pb_analyze.inc(1);
    }
    pb_analyze.finish_with_message("Analysis complete.");

    // Final Output
    processed_data.sort_by_key(|b| std::cmp::Reverse(b.height)); // Sort descending by height
    println!("{:->40}", "");
    println!("TOTAL BLOCKS: {}", processed_data.len());
    println!(
        "TOTAL LOSS:   ${:.2}",
        (total_loss_usd * 100.0).round() / 100.0
    );

    let output_file = "ocean_historical_report.json";
    let json_string = serde_json::to_string_pretty(&processed_data)?;
    tokio::fs::File::create(output_file)
        .await?
        .write_all(json_string.as_bytes())
        .await?;
    println!("Historical report saved to: {}", output_file);

    // Also write pools-3y.json
    let pools_3y_data = utils::fetch_from_mirror("/api/v1/mining/pools/3y", 0, 10).await?;
    let pools_3y_output_file = "pools-3y.json";
    let mut file = tokio::fs::File::create(pools_3y_output_file).await?;
    file.write_all(serde_json::to_string_pretty(&pools_3y_data)?.as_bytes())
        .await?;
    println!("Reference file {} updated.", pools_3y_output_file);

    Ok(())
}

pub async fn fetch_total_loss_ocean_report_rust() -> Result<()> {
    let slug = "ocean";
    let mut all_blocks: Vec<models::Block> = Vec::new();
    let mut last_height: Option<u64> = None;

    let total_expected_blocks = get_pool_stats_rust_total_loss().await?;

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

        let batch_val = utils::fetch_from_mirror(&path, 0, 10).await?;
        let batch: Vec<models::Block> = serde_json::from_value(batch_val)?;
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
    let mut join_set: tokio::task::JoinSet<Result<ProcessedBlockOutputTotalLoss, anyhow::Error>> =
        tokio::task::JoinSet::new();
    let processed_data: Vec<ProcessedBlockOutputTotalLoss> = Vec::new();
    let mut total_loss_usd = 0.0;

    println!(
        "
{:<10} | {:<10} | {:<10}",
        "Height", "Match Rate", "Loss (USD)"
    );
    println!("{:->40}", "");

    let pb_process = ProgressBar::new(all_blocks.len() as u64);
    pb_process.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}").unwrap()
        .progress_chars("#>- "));
    pb_process.set_message("Calculating Loss (USD)");

    for (_i, block) in all_blocks.into_iter().enumerate() {
        let cache_clone = price_cache.clone();
        join_set.spawn(async move {
            let timestamp = block.timestamp as i64;
            let extras = block.extras.unwrap_or(models::BlockExtras {
                match_rate: Some(0.0),
                reward: Some(0),
                expected_fees: Some(0),
            });

            let match_rate = extras.match_rate.unwrap_or(100.0);
            let actual_reward = extras.reward.unwrap_or(0);

            let expected_reward = if match_rate > 0.0 && match_rate < 100.0 {
                ((actual_reward as f64 * 100.0) / match_rate) as u64
            } else {
                actual_reward
            };
            let loss_sats = expected_reward.saturating_sub(actual_reward);

            #[allow(unused_assignments)]
            let mut hist_price = 0.0;
            if let Some(price) = cache_clone.get(&timestamp) {
                hist_price = *price;
            } else {
                // Fetch price if not in cache
                let price_path = format!(
                    "/api/v1/historical-price?timestamp={}&currency=USD",
                    timestamp
                );
                if let Ok(price_data_val) = utils::fetch_from_mirror(&price_path, 0, 5).await {
                    if let Some(usd_price) = price_data_val.get("usd").and_then(|u| u.as_f64()) {
                        hist_price = usd_price;
                        cache_clone.insert(timestamp, usd_price);
                    } else {
                        // Python uses a default of 74000.0 if price_data is None/empty, so we do
                        // too
                        hist_price = 74000.0;
                    }
                } else {
                    // Handle fetch_with_failover error for price
                    hist_price = 74000.0;
                }
            }

            let loss_usd = (loss_sats as f64 / 100_000_000.0) * hist_price;

            Ok(ProcessedBlockOutputTotalLoss {
                height: block.height,
                match_rate: (match_rate * 100.0).round() / 100.0, /* Python rounds to 2 decimal
                                                                   * places */
                loss_usd: (loss_usd * 100.0).round() / 100.0,
            })
        });
    }

    while let Some(res) = join_set.join_next().await {
        match res? {
            Ok(output) => {
                println!(
                    "{:<10} | {:<10.2} | {:<10.2}",
                    output.height, output.match_rate, output.loss_usd
                );
                total_loss_usd += output.loss_usd;
            }
            Err(e) => eprintln!("Error processing block: {}", e),
        }
        pb_process.inc(1);
    }
    pb_process.finish_with_message("Loss calculation complete.");

    println!("{:->40}", "");
    println!("TOTAL BLOCKS: {}", processed_data.len());
    println!(
        "TOTAL LOSS:   ${:.2}",
        (total_loss_usd * 100.0).round() / 100.0
    );

    // Save to file
    let output_file = "ocean_historical_report.json";
    let json_string = serde_json::to_string_pretty(&processed_data)?;
    let mut file = tokio::fs::File::create(output_file).await?;
    file.write_all(json_string.as_bytes()).await?;

    Ok(())
}

pub mod utils {
    use std::collections::HashMap;
    use std::sync::Arc;

    use anyhow::{Result, anyhow};
    use dashmap::DashMap;
    use regex::Regex;
    use reqwest::Client;
    use tokio::{io::AsyncWriteExt, time::Duration};

    use crate::{
        MIRRORS,
        models::{CoinbaseInfo, HistoricalPriceData, PriceData, Transaction},
    };
    pub async fn fetch_full_historical_prices_rust() -> Result<HashMap<i64, f64>> {
        let api_url = "https://mempool.space/api/v1/historical-price?currency=USD&timestamp=0";
        let output_file = "prices.json";

        println!(
            "--- Starting Full Historical BTC Price Fetch from {} ---",
            api_url
        );

        let response = Client::new()
            .get(api_url)
            .send()
            .await?
            .json::<HistoricalPriceData>()
            .await?;

        if response.prices.is_empty() {
            eprintln!("No historical price data received.");
            std::process::exit(1);
        }

        let mut file = tokio::fs::File::create(output_file).await?;
        file.write_all(serde_json::to_string_pretty(&response)?.as_bytes())
            .await?;

        println!("Full historical prices saved to: {}", output_file);

        let price_lookup: HashMap<i64, f64> = response
            .prices
            .into_iter()
            .filter_map(|p| p.usd.map(|usd_val| (p.time, usd_val)))
            .collect();
        Ok(price_lookup)
    }

    pub async fn fetch_from_mirror(
        path: &str,
        mirror_index: usize,
        timeout_secs: u64,
    ) -> Result<serde_json::Value> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;

        let mirrors_rotated = {
            let len = MIRRORS.len();
            let start = mirror_index % len;
            let mut rotated = Vec::with_capacity(len);
            for i in 0..len {
                rotated.push(MIRRORS[(start + i) % len]);
            }
            rotated
        };

        for base_url in mirrors_rotated {
            let url = format!("{}{}", base_url, path);
            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response.json().await?);
                    }
                    if response.status().as_u16() == 429 {
                        // Too many requests
                        continue;
                    }
                }
                Err(_) => {}
            }
        }
        Err(anyhow!(
            "Failed to fetch from all mirrors for path: {}",
            path
        ))
    }

    pub async fn fetch_block_transactions_rust(block_hash: &str) -> Result<CoinbaseInfo> {
        let path = format!("/api/block/{}/txs", block_hash);
        let txs_value = fetch_from_mirror(&path, 0, 10).await?;

        let transactions: Vec<Transaction> = serde_json::from_value(txs_value)?;

        if transactions.is_empty() {
            return Err(anyhow!("No transactions found for block {}", block_hash));
        }

        // The first transaction is typically the coinbase transaction
        let coinbase_tx = &transactions[0];

        let miner_name = coinbase_tx
            .vin
            .get(0)
            .and_then(|vin| vin.script_sig_asm.as_ref())
            .and_then(|script_sig_asm_str| {
                println!("DEBUG: script_sig_asm: {}", script_sig_asm_str);
                let re = match Regex::new(r"OP_PUSHBYTES_\d+ ([0-9a-fA-F]+)") {
                    Ok(r) => r,
                    Err(_) => return None,
                };
                for cap in re.captures_iter(script_sig_asm_str) {
                    let hex_data = &cap[1];
                    println!("DEBUG: Extracted scriptSig hex_data: {}", hex_data);
                    if let Ok(bytes) = hex::decode(hex_data) {
                        let decoded_string = String::from_utf8_lossy(&bytes);
                        println!(
                            "DEBUG: Decoded scriptSig string (lossy): {}",
                            decoded_string
                        );

                        // Heuristic: try to find common miner patterns in the decoded string
                        if decoded_string.contains("Ocean") {
                            return Some("Ocean Mining".to_string());
                        }
                        if decoded_string.contains("AntPool") {
                            return Some("AntPool".to_string());
                        }
                        if decoded_string.contains("Peak Mining") {
                            return Some("Peak Mining".to_string());
                        }
                        if decoded_string.contains("F2Pool") {
                            return Some("F2Pool".to_string());
                        }
                    }
                }
                None
            });

        // Extract OP_RETURN data
        let mut op_return_data: Vec<String> = Vec::new();
        let mut miner_name_from_op_return: Option<String> = None; // New variable

        for vout in &coinbase_tx.vout {
            if vout.scriptpubkey_type == "nulldata" && vout.scriptpubkey_asm.contains("OP_RETURN") {
                op_return_data.push(vout.scriptpubkey_asm.clone());
                println!(
                    "DEBUG: OP_RETURN scriptpubkey_asm: {}",
                    vout.scriptpubkey_asm
                );

                // Try to extract miner name from OP_RETURN data
                let re = match Regex::new(r"OP_PUSHBYTES_\d+ ([0-9a-fA-F]+)") {
                    Ok(r) => r,
                    Err(e) => return Err(anyhow!("Failed to create regex for OP_RETURN: {}", e)),
                };
                for cap in re.captures_iter(&vout.scriptpubkey_asm) {
                    let hex_data = &cap[1];
                    println!("DEBUG: Extracted OP_RETURN hex_data: {}", hex_data);
                    if let Ok(bytes) = hex::decode(hex_data) {
                        let decoded_string = String::from_utf8_lossy(&bytes);
                        println!(
                            "DEBUG: Decoded OP_RETURN string (lossy): {}",
                            decoded_string
                        );

                        if decoded_string.contains("Ocean") {
                            miner_name_from_op_return = Some("Ocean Mining".to_string());
                            break; // Found it, no need to check further in this vout
                        }
                        if decoded_string.contains("Peak Mining") {
                            miner_name_from_op_return = Some("Peak Mining".to_string());
                            break;
                        }
                        // Add other OP_RETURN specific miner heuristics here if
                        // needed
                    }
                }
            }
        }

        // Prioritize miner name from OP_RETURN if found, otherwise use scriptSig one
        let final_miner_name = miner_name_from_op_return.or(miner_name);

        Ok(CoinbaseInfo {
            miner_name: final_miner_name,
            op_return_data,
        })
    }

    pub async fn fetch_and_save_full_historical_prices() -> Result<()> {
        let output_file = "prices.json";

        let price_lookup: HashMap<i64, f64> = fetch_full_historical_prices_rust().await?;

        let historical_data = HistoricalPriceData {
            prices: price_lookup
                .into_iter()
                .map(|(time, usd)| PriceData {
                    time,
                    usd: Some(usd),
                })
                .collect(),
        };

        let json_string = serde_json::to_string_pretty(&historical_data)?;
        tokio::fs::File::create(output_file)
            .await?
            .write_all(json_string.as_bytes())
            .await?;

        println!("Full historical prices saved to: {}", output_file);

        Ok(())
    }

    pub async fn process_single_block(
        block: crate::models::Block,
        index: usize,
        price_cache: Arc<DashMap<i64, f64>>,
    ) -> Result<crate::models::ProcessedBlockOutput> {
        let timestamp = block.timestamp as i64;
        let extras = block.extras.unwrap_or(crate::models::BlockExtras {
            match_rate: Some(0.0),
            reward: Some(0),
            expected_fees: Some(0),
        });
        let match_rate = extras.match_rate.unwrap_or(100.0);
        let actual_reward = extras.reward.unwrap_or(0);

        let expected_reward = if match_rate > 0.0 && match_rate < 100.0 {
            ((actual_reward as f64 * 100.0) / match_rate) as u64
        } else {
            actual_reward
        };
        let loss_sats = expected_reward.saturating_sub(actual_reward);

        let mut hist_price = 0.0;
        if let Some(price) = price_cache.get(&timestamp) {
            hist_price = *price;
        } else {
            // Fetch price if not in cache
            let price_path = format!(
                "/api/v1/historical-price?timestamp={}&currency=USD",
                timestamp
            );
            if let Ok(price_data) = crate::utils::fetch_from_mirror(&price_path, index, 10).await {
                if let Some(usd_price) = price_data.get("usd").and_then(|u| u.as_f64()) {
                    hist_price = usd_price;
                    price_cache.insert(timestamp, usd_price);
                }
            }
        }

        let loss_usd = (loss_sats as f64 / 100_000_000.0) * hist_price;

        Ok(crate::models::ProcessedBlockOutput {
            height: block.height,
            match_rate: (match_rate * 100.0).round() / 100.0, // Python rounds to 2 decimal places
            loss_usd: (loss_usd * 100.0).round() / 100.0,
            price: (hist_price * 100.0).round() / 100.0,
        })
    }

    pub async fn get_pool_stats_rust() -> Result<u64> {
        let response = crate::utils::fetch_from_mirror("/api/v1/mining/pool/ocean", 0, 10).await?;
        let block_count = response
            .get("pool_stats")
            .and_then(|ps| ps.get("blockCount"))
            .and_then(|bc| bc.as_u64())
            .unwrap_or(832);
        Ok(block_count)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct BlockExtrasLib { // Renamed to avoid conflict with models::BlockExtras
    #[serde(rename = "matchRate")]
    pub match_rate: Option<f64>,
    pub reward: Option<u64>,
    #[serde(rename = "expectedFees")]
    pub expected_fees: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BlockLib { // Renamed to avoid conflict with models::Block
    pub height: u64,
    pub id: String,
    pub extras: Option<BlockExtrasLib>,
}

pub async fn fetch_blocks_sample(num_blocks: usize) -> Result<Vec<BlockLib>> {
    let url = "https://mempool.space/api/v1/mining/pool/ocean/blocks";
    let response = reqwest::get(url).await?.json::<Vec<BlockLib>>().await?;
    Ok(response.into_iter().take(num_blocks).collect())
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Pool {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(flatten)] // Capture other fields dynamically
    pub extra: serde_json::Value,
}

pub async fn fetch_and_save_pool_data() -> Result<()> {
    // Primary URL (from Python script, seems to be sweetsats.io first, then
    // mempool.space as fallback)
    let primary_url = "https://mempool.space/api/v1/mining/pools/1y"; // Corrected to mempool.space for consistency with actual usage, Python's var name was misleading
    let failover_url = "https://mempool.sweetsats.io/api/v1/mining/pools/1y"; // Actually mempool.space is fallback in Python, will use this as a reference if primary fails.

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
                println!(
                    "[-] {} returned {}. Trying failover URL...",
                    primary_url,
                    response.status()
                );
                let failover_response = reqwest::get(failover_url).await?;
                response_data = failover_response
                    .error_for_status()
                    .context(format!("HTTP error for failover URL {}", failover_url))?
                    .json::<Vec<Pool>>()
                    .await?;
                println!("[+] Successfully fetched from {}.", failover_url);
            }
        }
        Err(e) => {
            println!(
                "[-] Error fetching from {}: {}. Trying failover URL...",
                primary_url, e
            );
            let failover_response = reqwest::get(failover_url).await?;
            response_data = failover_response
                .error_for_status()
                .context(format!("HTTP error for failover URL {}", failover_url))?
                .json::<Vec<Pool>>()
                .await?;
            println!("[+] Successfully fetched from {}.", failover_url);
        }
    }

    // Write to JSON file
    let json_string = serde_json::to_string_pretty(&response_data)?;
    tokio::fs::File::create(output_file)
        .await?
        .write_all(json_string.as_bytes())
        .await?;

    println!(
        "[+] Successfully wrote {} pool entries to {}",
        response_data.len(),
        output_file
    );

    Ok(())
}

pub async fn fetch_ocean_data_rust() -> anyhow::Result<()> {
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
        match crate::utils::fetch_from_mirror(&path, 0, 10).await {
            Ok(data) => {
                println!(
                    "
[+] {}:",
                    title
                );
                if data.is_array() {
                    println!("{}", serde_json::to_string_pretty(&data)?);
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

pub fn generate_ocean_config_env_rust() {
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

#[derive(Parser, Debug)]
#[clap(author, version, about = "Fetch and analyze OCEAN mining pool data.")]
pub struct Args {
    /// Number of sample blocks to print.
    #[clap(long, default_value = "1000")]
    pub depth: usize,
}

pub async fn fetch_all_ocean_blocks_rust(depth_limit: usize) -> Result<()> {
    #[allow(unused_assignments)] // Suppress warning for best_diff
    let slug = "ocean";
    let base_blocks_url = format!("https://mempool.space/api/v1/mining/pool/{}/blocks", slug);

    let mut all_blocks: Vec<models::Block> = Vec::new();
    let mut last_height: Option<u64> = None;

    let price_file_name = "prices.json".to_string(); // Define as String
    let price_lookup_map: HashMap<i64, f64> = match tokio::fs::File::open(&price_file_name).await {
        Ok(_file) => {
            let content = tokio::fs::read_to_string(&price_file_name).await?;
            let historical_data: models::HistoricalPriceData = serde_json::from_str(&content)?;
            let price_map: HashMap<i64, f64> = historical_data
                .prices
                .into_iter()
                .filter_map(|p| p.usd.map(|usd_val| (p.time, usd_val)))
                .collect();
            println!(
                "Loaded {} historical prices from {}",
                price_map.len(),
                price_file_name
            );
            price_map
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "{} not found. Attempting to fetch full historical prices...",
                price_file_name
            );
            let price_map = crate::utils::fetch_full_historical_prices_rust().await?;
            println!(
                "Loaded {} historical prices from {} (after fetch)",
                price_map.len(),
                price_file_name
            );
            price_map
        }
        Err(e) => return Err(anyhow!("Error opening {}: {}", price_file_name, e)),
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
        let batch: Vec<models::Block> = response
            .error_for_status()
            .context(format!("HTTP error fetching blocks from {}", url))?
            .json::<Vec<models::Block>>()
            .await?;

        if batch.is_empty() {
            break;
        }

        all_blocks.extend(batch.into_iter());
        last_height = Some(all_blocks.last().unwrap().height);

        println!(
            "Fetched {} blocks... (Current Height: {})",
            all_blocks.len(),
            last_height.unwrap()
        );

        sleep(Duration::from_millis(500)).await; // Python uses 0.5s sleep
    }

    // 2. Process and Calculate
    let mut total_loss_usd = 0.0;
    let mut processed_data: Vec<models::ProcessedBlockData> = Vec::new();

    println!(
        "
{:<10} | {:<8} | {:<12} | {:<10}",
        "Height", "Health", "Loss(丰)", "Loss($)"
    );
    println!("{:->50}", "");

    for (_i, b) in all_blocks.iter().enumerate() {
        let match_rate = b
            .extras
            .as_ref()
            .and_then(|e| e.match_rate)
            .unwrap_or(0.0)
            .round();
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
                    closest_price = sorted_timestamps
                        .get(exact_idx)
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

        let processed_block = models::ProcessedBlockData {
            height: b.height,
            health: match_rate,
            loss_sats,
            loss_usd: (loss_usd * 100.0).round() / 100.0,
            btc_usd: (btc_usd * 100.0).round() / 100.0,
        };
        processed_data.push(processed_block.clone());

        // Print a sample of the first few
        if processed_data.len() <= depth_limit {
            println!(
                "{:<10} | {:>6.2}% | {:<12} | ${:>8.2}",
                processed_block.height,
                processed_block.health,
                processed_block.loss_sats,
                processed_block.loss_usd
            );
        }

        // Fetch and display coinbase info
        match crate::utils::fetch_block_transactions_rust(&b.id).await {
            Ok(coinbase_info) => {
                println!(
                    "    Block {}: Miner: {}",
                    b.height,
                    coinbase_info
                        .miner_name
                        .unwrap_or_else(|| "Unknown Miner".to_string())
                );
                if !coinbase_info.op_return_data.is_empty() {
                    for op_ret in coinbase_info.op_return_data {
                        println!("        OP_RETURN: {}", op_ret);
                    }
                }
            }
            Err(e) => eprintln!(
                "    Error fetching coinbase info for block {}: {}",
                b.height, e
            ),
        }
    }

    // 3. Output Summary
    println!("{:->50}", "");
    println!("TOTAL BLOCKS MINED: {}", all_blocks.len());
    println!(
        "TOTAL CUMULATIVE LOSS: ${:.2}",
        (total_loss_usd * 100.0).round() / 100.0
    );

    // Save to file
    let output_file = "ocean_full_history.json";
    let json_string = serde_json::to_string_pretty(&processed_data)?;
    tokio::fs::File::create(output_file)
        .await?
        .write_all(json_string.as_bytes())
        .await?;
    println!(
        "
Full dataset saved to: {}",
        output_file
    );

    Ok(())
}


