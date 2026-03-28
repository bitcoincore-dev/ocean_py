// Reusable functions and structs for the ocean_py project.

const MIRRORS: &[&str] = &[
    "https://mempool.space",
    "https://mempool.sweetsats.io"
];

pub mod models {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct PriceData {
        pub time: i64,
        #[serde(rename = "USD")]
        pub usd: f64,
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
}

pub mod utils {
    use anyhow::Result;
    use std::collections::HashMap;
    use tokio::io::AsyncWriteExt;
    use crate::models::HistoricalPriceData;
    use crate::MIRRORS;
    use reqwest;
    use serde_json::Value;

    pub async fn fetch_full_historical_prices_rust() -> Result<HashMap<i64, f64>> {
        let api_url = "https://mempool.space/api/v1/historical-price?currency=USD&timestamp=0";
        let output_file = "prices.json";

        println!("--- Starting Full Historical BTC Price Fetch from {} ---", api_url);

        let response = reqwest::get(api_url).await?.json::<HistoricalPriceData>().await?;

        if response.prices.is_empty() {
            eprintln!("No historical price data received.");
            std::process::exit(1);
        }

        let mut file = tokio::fs::File::create(output_file).await?;
        file.write_all(serde_json::to_string_pretty(&response)?.as_bytes()).await?;

        println!("Full historical prices saved to: {}", output_file);

        let price_lookup: HashMap<i64, f64> = response.prices.into_iter().map(|p| (p.time, p.usd)).collect();
        Ok(price_lookup)
    }

    pub async fn fetch_from_mirror(path: &str, mirror_index: usize) -> Result<serde_json::Value> {
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
            match reqwest::get(&url).await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response.json().await?);
                    }
                },
                Err(_) => {},
            }
        }
        Err(anyhow::anyhow!("Failed to fetch from all mirrors for path: {}", path))
    }
}
