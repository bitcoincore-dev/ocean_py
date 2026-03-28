use reqwest;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PriceData {
    time: i64,
    #[serde(rename = "USD")]
    usd: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct HistoricalPriceData {
    prices: Vec<PriceData>,
}

async fn fetch_and_save_full_historical_prices() -> Result<()> {
    let api_url = "https://mempool.space/api/v1/historical-price?currency=USD&timestamp=0";
    let output_file = "prices.json";

    println!("--- Starting Full Historical BTC Price Fetch from {} ---", api_url);

    let response = reqwest::get(api_url).await?.json::<HistoricalPriceData>().await?;

    if response.prices.is_empty() {
        eprintln!("No historical price data received.");
        std::process::exit(1);
    }

    let json_string = serde_json::to_string_pretty(&response)?;
    tokio::fs::File::create(output_file).await?.write_all(json_string.as_bytes()).await?;

    println!("Full historical prices saved to: {}", output_file);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    fetch_and_save_full_historical_prices().await
}
