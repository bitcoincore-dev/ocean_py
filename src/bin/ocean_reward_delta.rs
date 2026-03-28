use anyhow::Result;
use ocean_loss_estimator_rs::models::{Block, BlockExtras, PoolData};
use ocean_loss_estimator_rs::utils::fetch_from_mirror;
use serde::Deserialize; // Still needed for CurrentPrice
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct CurrentPrice {
    #[serde(rename = "USD")]
    usd: f64,
}

async fn get_ocean_reward_delta_rust() -> Result<()> {
    let slug = "ocean";
    let pool_path = format!("/api/v1/mining/pool/{}", slug);
    let blocks_path = format!("/api/v1/mining/pool/{}/blocks", slug);
    let price_path = "/api/v1/prices";

    // 1. Fetch Current BTC Price (USD)
    let price_data: Value = fetch_from_mirror(&price_path, 0, 10).await?;
    let price_res: CurrentPrice = serde_json::from_value(price_data)?;
    let btc_usd = price_res.usd;

    // 2. Get Aggregate Pool Health
    let pool_data: Value = fetch_from_mirror(&pool_path, 0, 10).await?;
    let pool_res: PoolData = serde_json::from_value(pool_data)?;
    let avg_health = pool_res.avg_block_health.unwrap_or_default();

    // 3. Get Individual Block Data
    let blocks_data: Value = fetch_from_mirror(&blocks_path, 0, 10).await?;
    let block_res: Vec<Block> = serde_json::from_value(blocks_data)?;

    println!("--- OCEAN Pool Metrics (BTC Price: ${:.2}) ---", btc_usd);
    println!("Aggregate Pool Health: {}%", avg_health);
    println!("
{:->90}", "");
    println!("{:<10} | {:<8} | {:<15} | {:<15} | {:<10}",
             "Height", "Health", "Actual (Sats)", "Expected (Sats)", "Loss (USD)");
    println!("{:->90}", "");

    for b in block_res.iter().take(10) { // Limiting display to 10 for readability
        let height = b.height;
        let extras = b.extras.as_ref().unwrap_or(&BlockExtras { match_rate: Some(0.0), reward: Some(0), expected_fees: Some(0) });

        let match_rate = extras.match_rate.unwrap_or(0.0);
        let actual_reward = extras.reward.unwrap_or(0);

        let expected_reward = if match_rate > 0.0 {
            (actual_reward as f64 / (match_rate / 100.0)) as u64
        } else {
            actual_reward
        };

        let diff_sats = expected_reward.saturating_sub(actual_reward);
        let diff_usd = (diff_sats as f64 / 100_000_000.0) * btc_usd;

        println!("{:<10} | {:>6.2}% | {:>15} | {:>15} | ${:>8.2}",
                 height, match_rate, actual_reward, expected_reward, diff_usd);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    get_ocean_reward_delta_rust().await
}
