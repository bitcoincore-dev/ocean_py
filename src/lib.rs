extern crate hex;

// Reusable functions and structs for the ocean_py project.

const MIRRORS: &[&str] = &[
    "https://mempool.space",
    "https://mempool.sweetsats.io"
];

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
}

pub mod utils {
    use anyhow::{Result, anyhow};
    use std::collections::HashMap;
    use tokio::io::AsyncWriteExt;
    use crate::models::{HistoricalPriceData, Transaction, CoinbaseInfo};
    use crate::MIRRORS;
    use reqwest::Client;
    use serde_json::Value;
    use tokio::time::Duration;
    use regex::Regex;

    pub async fn fetch_full_historical_prices_rust() -> Result<HashMap<i64, f64>> {
        let api_url = "https://mempool.space/api/v1/historical-price?currency=USD&timestamp=0";
        let output_file = "prices.json";

        println!("--- Starting Full Historical BTC Price Fetch from {} ---", api_url);

        let response = Client::new().get(api_url).send().await?.json::<HistoricalPriceData>().await?;

        if response.prices.is_empty() {
            eprintln!("No historical price data received.");
            std::process::exit(1);
        }

        let mut file = tokio::fs::File::create(output_file).await?;
        file.write_all(serde_json::to_string_pretty(&response)?.as_bytes()).await?;

        println!("Full historical prices saved to: {}", output_file);

        let price_lookup: HashMap<i64, f64> = response.prices.into_iter().filter_map(|p| p.usd.map(|usd_val| (p.time, usd_val))).collect();
        Ok(price_lookup)
    }

    pub async fn fetch_from_mirror(path: &str, mirror_index: usize, timeout_secs: u64) -> Result<serde_json::Value> {
        let client = Client::builder().timeout(Duration::from_secs(timeout_secs)).build()?;

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
                    if response.status().as_u16() == 429 { // Too many requests
                        continue;
                    }
                },
                Err(_) => {},
            }
        }
        Err(anyhow!("Failed to fetch from all mirrors for path: {}", path))
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

        let miner_name = coinbase_tx.vin.get(0)
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
                        println!("DEBUG: Decoded scriptSig string (lossy): {}", decoded_string);

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
                println!("DEBUG: OP_RETURN scriptpubkey_asm: {}", vout.scriptpubkey_asm);

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
                        println!("DEBUG: Decoded OP_RETURN string (lossy): {}", decoded_string);

                        if decoded_string.contains("Ocean") {
                            miner_name_from_op_return = Some("Ocean Mining".to_string());
                            break; // Found it, no need to check further in this vout
                        }
                        if decoded_string.contains("Peak Mining") {
                            miner_name_from_op_return = Some("Peak Mining".to_string());
                            break;
                        }
                        // Add other OP_RETURN specific miner heuristics here if needed
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
}

use anyhow::{Result, Context};
use serde::Deserialize;
use reqwest;
use tokio::time::{sleep, Duration};

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
