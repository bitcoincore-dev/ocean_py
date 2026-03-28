extern crate hex;

// Reusable functions and structs for the ocean_py project.

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

use anyhow::{Context, Result};
use reqwest;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Deserialize, Clone)]
pub struct BlockExtras {
    #[serde(rename = "matchRate")]
    pub match_rate: Option<f64>,
    pub reward: Option<u64>,
    #[serde(rename = "expectedFees")]
    pub expected_fees: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Block {
    pub height: u64,
    pub id: String,
    pub extras: Option<BlockExtras>,
}

pub async fn fetch_blocks_sample(num_blocks: usize) -> Result<Vec<Block>> {
    let url = "https://mempool.space/api/v1/mining/pool/ocean/blocks";
    let response = reqwest::get(url).await?.json::<Vec<Block>>().await?;
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
    let primary_url = "https://mempool.space/api/v1/mining/pools/1y"; // Corrected to mempool.space for consistency with actual usage, Python\'s var name was misleading
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
