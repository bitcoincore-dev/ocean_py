use anyhow::Result;
use ocean_loss_estimator_rs::{models::Block, utils::fetch_from_mirror};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct PoolData {
    #[serde(rename = "avgBlockHealth")]
    avg_block_health: Option<f64>,
    // Add other fields from pool API if needed
}

async fn get_ocean_health_rust() -> Result<()> {
    let slug = "ocean";
    let pool_path = format!("/api/v1/mining/pool/{}", slug);
    let blocks_path = format!("/api/v1/mining/pool/{}/blocks", slug);

    // 1. Get Aggregate Pool Health
    let pool_data: Value = fetch_from_mirror(&pool_path, 0, 10).await?;
    let pool_res: PoolData = serde_json::from_value(pool_data)?;
    let avg_health = pool_res.avg_block_health.unwrap_or_default();

    // 2. Get Individual Block Health (Match Rate)
    let blocks_data: Value = fetch_from_mirror(&blocks_path, 0, 10).await?;
    let blocks_res: Vec<Block> = serde_json::from_value(blocks_data)?;

    println!("--- OCEAN Pool Health Metrics ---");
    println!("Aggregate Pool Health (avgBlockHealth): {}%", avg_health);
    println!(
        "
--- Recent Block Health (matchRate) ---"
    );

    // Python shows up to 10000 blocks, we'll stick to a reasonable default or fetch
    // all if not too many
    for b in blocks_res.iter().take(10000) {
        let height = b.height;
        let match_rate = b
            .extras
            .as_ref()
            .and_then(|e| e.match_rate)
            .unwrap_or_default();
        println!("Block Height: {} | Health: {}%", height, match_rate);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    get_ocean_health_rust().await
}
