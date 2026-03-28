use reqwest;
use serde::Deserialize;
use anyhow::{Result, Context};

#[derive(Debug, Deserialize)]
struct PoolData {
    #[serde(rename = "avgBlockHealth")]
    avg_block_health: Option<f64>,
    // Add other fields from pool API if needed
}

#[derive(Debug, Deserialize)]
struct BlockExtras {
    #[serde(rename = "matchRate")]
    match_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct Block {
    height: u64,
    extras: Option<BlockExtras>,
}

async fn get_ocean_health_rust() -> Result<()> {
    let slug = "ocean";
    let pool_url = format!("https://mempool.space/api/v1/mining/pool/{}", slug);
    let blocks_url = format!("https://mempool.space/api/v1/mining/pool/{}/blocks", slug);

    // 1. Get Aggregate Pool Health
    let pool_res = reqwest::get(&pool_url).await?.json::<PoolData>().await?;
    let avg_health = pool_res.avg_block_health.unwrap_or_default();

    // 2. Get Individual Block Health (Match Rate)
    let blocks_res = reqwest::get(&blocks_url).await?.json::<Vec<Block>>().await?;

    println!("--- OCEAN Pool Health Metrics ---");
    println!("Aggregate Pool Health (avgBlockHealth): {}%", avg_health);
    println!("
--- Recent Block Health (matchRate) ---");

    // Python shows up to 10000 blocks, we'll stick to a reasonable default or fetch all if not too many
    for b in blocks_res.iter().take(10000) {
        let height = b.height;
        let match_rate = b.extras.as_ref().and_then(|e| e.match_rate).unwrap_or_default();
        println!("Block Height: {} | Health: {}%", height, match_rate);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    get_ocean_health_rust().await
}
