use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt; // Add this import
//use serde::Serialize;
use tokio::time::{Duration, sleep};

const MIRRORS: &[&str] = &["https://mempool.space", "https://mempool.sweetsats.io"];

use ocean_loss_estimator_rs::{
    models::{Block, BlockExtras, ProcessedBlockOutput},
    utils::fetch_from_mirror,
};

use ocean_loss_estimator_rs::utils::get_pool_stats_rust;

async fn process_single_block(
    block: Block,
    index: usize,
    price_cache: Arc<DashMap<i64, f64>>,
) -> Result<ProcessedBlockOutput> {
    let timestamp = block.timestamp as i64;
    let extras = block.extras.unwrap_or(BlockExtras {
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
        if let Ok(price_data) = fetch_from_mirror(&price_path, index, 10).await {
            if let Some(usd_price) = price_data.get("usd").and_then(|u| u.as_f64()) {
                hist_price = usd_price;
                price_cache.insert(timestamp, usd_price);
            }
        }
    }

    let loss_usd = (loss_sats as f64 / 100_000_000.0) * hist_price;

    Ok(ProcessedBlockOutput {
        height: block.height,
        match_rate: (match_rate * 100.0).round() / 100.0, // Python rounds to 2 decimal places
        loss_usd: (loss_usd * 100.0).round() / 100.0,
        price: (hist_price * 100.0).round() / 100.0,
    })
}

async fn fetch_full_ocean_report_rust() -> Result<()> {
    println!("--- Parallel OCEAN Audit ---");

    let total_expected_blocks = get_pool_stats_rust().await?;
    let mut all_blocks: Vec<Block> = Vec::new();
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

        let batch: Vec<Block> = serde_json::from_value(fetch_from_mirror(&path, 0, 10).await?)?;

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
    let mut processed_data: Vec<ProcessedBlockOutput> = Vec::new();
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
        join_set.spawn(async move { process_single_block(block, i, cache_clone).await });
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
    let pools_3y_data = fetch_from_mirror("/api/v1/mining/pools/3y", 0, 10).await?;
    let pools_3y_output_file = "pools-3y.json";
    let mut file = tokio::fs::File::create(pools_3y_output_file).await?;
    file.write_all(serde_json::to_string_pretty(&pools_3y_data)?.as_bytes())
        .await?;
    println!("Reference file {} updated.", pools_3y_output_file);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    fetch_full_ocean_report_rust().await
}
