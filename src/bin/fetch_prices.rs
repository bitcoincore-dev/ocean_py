use anyhow::Result;
use tokio::io::AsyncWriteExt;

use ocean_loss_estimator_rs::models::{HistoricalPriceData, PriceData};
use ocean_loss_estimator_rs::utils::fetch_full_historical_prices_rust;
use std::collections::HashMap;

async fn fetch_and_save_full_historical_prices() -> Result<()> {
    let output_file = "prices.json";

    let price_lookup: HashMap<i64, f64> = fetch_full_historical_prices_rust().await?;

    let historical_data = HistoricalPriceData { 
        prices: price_lookup.into_iter().map(|(time, usd)| PriceData { time, usd: Some(usd) }).collect()
    };

    let json_string = serde_json::to_string_pretty(&historical_data)?;
    tokio::fs::File::create(output_file).await?.write_all(json_string.as_bytes()).await?;

    println!("Full historical prices saved to: {}", output_file);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    fetch_and_save_full_historical_prices().await
}
