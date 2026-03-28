use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = "Estimate cumulative loss for mining pools using Ocean's rules. Note: '丰' is the Unicode symbol for Satoshis.")]
struct Args {
    /// The slug of the reference mining pool (default: ocean)
    #[clap(long, default_value = "ocean")]
    ocean_slug: String,

    /// Comma-separated list of other mining pool slugs to analyze (e.g., antpool,f2pool)
    #[clap(long)]
    other_pools: Option<String>,

    /// Number of historical blocks to fetch and analyze for each pool. Analyzes all if not specified.
    #[clap(long)]
    depth: Option<usize>,

    /// Enable verbose output for debugging and detailed progress.
    #[clap(long)]
    verbose: bool,

    /// Force an update of prices.json from mempool.space, regardless of whether it exists.
    #[clap(long)]
    update: bool,
}

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

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PoolBlockExtras {
    #[serde(rename = "matchRate")]
    match_rate: Option<f64>,
    reward: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PoolBlock {
    height: u64,
    timestamp: u64,
    extras: Option<PoolBlockExtras>,
}

#[derive(Debug, Deserialize, Serialize)]
struct MempoolPool {
    slug: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct MempoolPoolsResponse {
    pools: Vec<MempoolPool>,
}

struct ProcessedBlockData {
    height: u64,
    health: f64,
    actual_reward: u64,
    expected_reward: u64,
    loss_sats: u64,
    loss_usd: f64,
    btc_usd: f64,
    timestamp: u64,
}

struct SummaryResult {
    pool_slug: String,
    total_estimated_loss_usd: f64,
    comparisons_made: usize,
}

async fn fetch_mempool_pools(time_period: &str, limit: Option<usize>, args: &Args) -> Result<Vec<String>> {
    let api_url = format!("https://mempool.space/api/v1/mining/pools/{}", time_period);
    if args.verbose {
        println!("--- Fetching active pools from {} ---", api_url);
    }

    let response = reqwest::get(&api_url).await?.json::<MempoolPoolsResponse>().await?;
    let mut pool_slugs: Vec<String> = response.pools.into_iter().map(|p| p.slug).collect();

    if let Some(l) = limit {
        if pool_slugs.len() > l {
            if args.verbose {
                println!("Limiting fetched pools to {}.", l);
            }
            pool_slugs.truncate(l);
        }
    }

    if args.verbose {
        println!("Fetched {} pools from mempool.space.", pool_slugs.len());
    }
    Ok(pool_slugs)
}

async fn fetch_full_historical_prices(args: &Args) -> Result<HashMap<i64, f64>> {
    let api_url = "https://mempool.space/api/v1/historical-price?currency=USD&timestamp=0";

    if args.verbose {
        println!("--- Starting Full Historical BTC Price Fetch from {} ---", api_url);
    }

    let response = reqwest::get(api_url).await?.json::<HistoricalPriceData>().await?;

    if response.prices.is_empty() {
        eprintln!("No historical price data received.");
        std::process::exit(1);
    }

    let price_lookup: HashMap<i64, f64> = response.prices.into_iter().map(|p| (p.time, p.usd)).collect();
    Ok(price_lookup)
}

async fn analyze_pool_loss(
    pool_slug: &str,
    depth: Option<usize>,
    args: &Args,
    price_lookup_map: &HashMap<i64, f64>,
) -> Result<Vec<ProcessedBlockData>> {
    let base_blocks_url = format!("https://mempool.space/api/v1/mining/pool/{}/blocks", pool_slug);
    let mut all_blocks: Vec<PoolBlock> = Vec::new();
    let mut last_height: Option<u64> = None;

    if args.verbose {
        println!("--- Starting Full History Crawl for {} ---", pool_slug.to_uppercase());
    }

    loop {
        let url = match last_height {
            Some(h) => format!("{}/{}", base_blocks_url, h),
            None => base_blocks_url.clone(),
        };

        let batch: Vec<PoolBlock> = reqwest::get(&url).await?.json().await?;

        if batch.is_empty() {
            break;
        }

        all_blocks.extend(batch.into_iter());
        last_height = Some(all_blocks.last().unwrap().height);

        if args.verbose {
            println!("Fetched {} blocks... (Current Height: {})", all_blocks.len(), last_height.unwrap());
        }

        if let Some(d) = depth {
            if all_blocks.len() >= d {
                all_blocks.truncate(d);
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    let mut processed_data: Vec<ProcessedBlockData> = Vec::new();

    let sorted_timestamps: Vec<i64> = price_lookup_map.keys().copied().collect(); // Collect keys to sort
    let mut sorted_timestamps_indexed: Vec<(usize, i64)> = sorted_timestamps.into_iter().enumerate().collect();
    sorted_timestamps_indexed.sort_by_key(|&(_, ts)| ts);

    for b in all_blocks {
        let extras = b.extras.unwrap_or(PoolBlockExtras { match_rate: Some(0.0), reward: Some(0) });
        
        let match_rate = extras.match_rate.unwrap_or(0.0).round();
        let actual_reward = extras.reward.unwrap_or(0);

        let expected_reward = if match_rate > 0.0 && match_rate < 100.0 {
            ((actual_reward as f64 * 100.0) / match_rate) as u64
        } else {
            actual_reward
        };
        let loss_sats = expected_reward.saturating_sub(actual_reward);

        let timestamp = b.timestamp as i64;

        if args.verbose {
            println!("DEBUG: Block height: {}, Raw timestamp: {}, Query timestamp: {}", b.height, b.timestamp, timestamp);
        }

        let btc_usd = {
            let mut closest_timestamp: Option<i64> = None;
            for &(_, hist_ts) in &sorted_timestamps_indexed {
                if hist_ts <= timestamp {
                    closest_timestamp = Some(hist_ts);
                } else {
                    break;
                }
            }
            closest_timestamp.and_then(|ts| price_lookup_map.get(&ts).copied()).unwrap_or(0.0)
        };

        let loss_usd = (loss_sats as f64 / 100_000_000.0) * btc_usd;

        processed_data.push(ProcessedBlockData {
            height: b.height,
            health: match_rate,
            actual_reward,
            expected_reward,
            loss_sats,
            loss_usd: (loss_usd * 100.0).round() / 100.0,
            btc_usd: (btc_usd * 100.0).round() / 100.0,
            timestamp: b.timestamp,
        });
    }

    Ok(processed_data)
}

fn print_ocean_health_table(ocean_slug: &str, ocean_processed_data: &[ProcessedBlockData]) {
    println!("
--- OCEAN Health Report for {} ---", ocean_slug.to_uppercase());
    println!("{:<8} | {:<10} | {:<11} | {:<11} | {:<10} | {:<11} | {:<12} | {:<10}",
        "Height", "Time(UTC)", "Exp. Reward", "Reward($)", "Health(%)", "Loss(丰)", "Loss($)", "BTC/USD");
    println!("{:->104}", "");

    for block in ocean_processed_data {
        println!("{:<8} | {:<10} | {:<11} | {:<11} | {:<10.2} | {:<11} | {:<12.2} | {:<10.2}",
            block.height, block.timestamp, block.expected_reward, block.actual_reward, block.health,
            block.loss_sats, block.loss_usd, block.btc_usd);
    }
    println!("{:->104}", "");
}

async fn compare_pool_losses(
    ocean_slug: &str,
    other_pool_slugs: &[String],
    depth: Option<usize>,
    args: &Args,
    price_lookup_map: &HashMap<i64, f64>,
) -> Result<Vec<SummaryResult>> {
    if args.verbose {
        println!("
--- Comparing Losses: {} vs. {} ---",
                 ocean_slug.to_uppercase(), other_pool_slugs.iter().map(|s| s.to_uppercase()).collect::<Vec<_>>().join(", "));
    }

    // 1. Analyze Ocean's actual losses
    if args.verbose {
        println!("
Analyzing {}...", ocean_slug.to_uppercase());
    }
    let ocean_processed_data = analyze_pool_loss(ocean_slug, depth, args, price_lookup_map).await?;

    // Print Ocean's health table
    print_ocean_health_table(ocean_slug, &ocean_processed_data);

    // Determine the actual number of blocks fetched for Ocean
    let actual_ocean_depth = ocean_processed_data.len();
    if args.verbose {
        println!("Ocean analyzed {} blocks.", actual_ocean_depth);
    }

    // Calculate Ocean's total loss for reference
    let ocean_total_loss_usd: f64 = ocean_processed_data.iter().map(|b| b.loss_usd).sum();
    println!("TOTAL CUMULATIVE LOSS for {}: ${:.2}", ocean_slug.to_uppercase(), ocean_total_loss_usd);

    let mut summary_results: Vec<SummaryResult> = Vec::new();

    for other_pool_slug in other_pool_slugs {
        if other_pool_slug == ocean_slug {
            println!("
Skipping loss estimation for {} as it is the reference pool.", ocean_slug.to_uppercase());
            continue;
        }
        println!("
Analyzing {} (limited to {} blocks)...
", other_pool_slug.to_uppercase(), actual_ocean_depth);
        
        let other_pool_processed_data = analyze_pool_loss(other_pool_slug, Some(actual_ocean_depth), args, price_lookup_map).await?;

        let mut other_pool_data_by_timestamp: Vec<(usize, &ProcessedBlockData)> = other_pool_processed_data.iter().enumerate().collect();
        other_pool_data_by_timestamp.sort_by_key(|&(_, b)| b.timestamp);

        let mut other_pool_blocks_used = vec![false; other_pool_data_by_timestamp.len()]; // Track used blocks

        let mut estimated_other_pool_loss_usd = 0.0;
        let mut comparisons_made = 0;
        const TIME_DIFFERENCE_THRESHOLD: i64 = 3600; // 1 hour in seconds

        println!("{:^47} | {:^47}", "OCEAN", other_pool_slug.to_uppercase());
        println!("{:<8} | {:<10} | {:<10} | {:<10} | {:<8} | {:<10} | {:<10} | {:<10}",
            "Height", "Time(UTC)", "Loss($)", "Reward($)", "Height", "Time(UTC)", "Reward($)", "Est. Loss($)");
        println!("{:->97}", "");

        for ocean_block in &ocean_processed_data {
            if ocean_block.loss_sats > 0 && ocean_block.expected_reward > 0 {
                let ocean_loss_quotient = ocean_block.loss_sats as f64 / ocean_block.expected_reward as f64;
                let ocean_block_timestamp = ocean_block.timestamp as i64;

                let mut closest_other_block: Option<&ProcessedBlockData> = None;
                let mut closest_other_block_index: isize = -1;
                let mut min_time_diff = i64::MAX;

                for (other_block_idx, other_block) in other_pool_data_by_timestamp.iter().enumerate() {
                    if other_pool_blocks_used[other_block_idx] {
                        continue; // Skip if this block has already been used
                    }

                    let other_block_timestamp = other_block.1.timestamp as i64;
                    let time_diff = (ocean_block_timestamp - other_block_timestamp).abs();

                    if time_diff < min_time_diff {
                        min_time_diff = time_diff;
                        closest_other_block = Some(other_block.1);
                        closest_other_block_index = other_block_idx as isize;
                    }
                    // Optimization: if current other_block_timestamp is already much larger than
                    // ocean_block_timestamp, and we are iterating in sorted order, we can break.
                    // Re-enabling the optimization with a check to ensure at least one block was found
                    if other_block_timestamp > ocean_block_timestamp + TIME_DIFFERENCE_THRESHOLD && min_time_diff != i64::MAX {
                         break;
                    }
                }

                if let Some(closest_other_block) = closest_other_block {
                    if closest_other_block_index != -1 && min_time_diff <= TIME_DIFFERENCE_THRESHOLD {
                        // Mark the closest block as used
                        other_pool_blocks_used[closest_other_block_index as usize] = true;
                        // Estimate loss for the other pool's block
                        let other_pool_estimated_loss_sats = ocean_loss_quotient * closest_other_block.actual_reward as f64;
                        let other_pool_estimated_loss_usd = (other_pool_estimated_loss_sats / 100_000_000.0) * closest_other_block.btc_usd;
                        estimated_other_pool_loss_usd += other_pool_estimated_loss_usd;
                        comparisons_made += 1;

                        let ocean_actual_usd = (ocean_block.actual_reward as f64 / 100_000_000.0) * ocean_block.btc_usd;
                        let other_pool_actual_usd = (closest_other_block.actual_reward as f64 / 100_000_000.0) * closest_other_block.btc_usd;
                        
                        println!("{:<8} | {:<10} | {:<10.2} | {:<10.2} | {:<8} | {:<10} | {:<10.2} | {:<10.2}",
                            ocean_block.height, ocean_block.timestamp, ocean_block.loss_usd, ocean_actual_usd,
                            closest_other_block.height, closest_other_block.timestamp, other_pool_actual_usd, other_pool_estimated_loss_usd);
                    }
                }
            }
        }

        println!("{:->97}", "");
        println!("TOTAL ESTIMATED CUMULATIVE LOSS for {}: ${:.2} ({} blocks compared)",
                 other_pool_slug.to_uppercase(), estimated_other_pool_loss_usd, comparisons_made);
        // Append to summary_results instead of printing directly
        summary_results.push(SummaryResult {
            pool_slug: other_pool_slug.to_uppercase(),
            total_estimated_loss_usd: estimated_other_pool_loss_usd,
            comparisons_made,
        });
    }

    Ok(summary_results)
}

fn print_summary_table(summary_results: &[SummaryResult]) {
    println!("
--- SUMMARY OF ESTIMATED LOSSES ---");
    println!("{:<15} | {:<15} | {:<18}", "Pool", "Est. Loss($)", "Blocks Compared");
    println!("{:->53}", "");

    for result in summary_results {
        println!("{:<15} | {:<15.2} | {:<18}",
            result.pool_slug, result.total_estimated_loss_usd, result.comparisons_made);
    }
    println!("{:->53}", "");
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let price_lookup_map: HashMap<i64, f64>;
    let output_file = "prices.rs.json";

    if args.update {
        if args.verbose {
            println!("DEBUG: --update flag is set. Forcing price data fetch.");
        }
        price_lookup_map = fetch_full_historical_prices(&args).await?;
        let mut historical_data = HistoricalPriceData { prices: price_lookup_map.iter().map(|(&time, &usd)| PriceData { time, usd }).collect() };
        historical_data.prices.sort_by_key(|p| p.time); // Sort by timestamp
        let mut file = std::fs::File::create(output_file)?;
        serde_json::to_writer_pretty(&mut file, &historical_data)?;
        if args.verbose {
            println!("Full historical prices saved to: {}. Loaded {} entries.", output_file, price_lookup_map.len());
        }
    } else {
        match std::fs::File::open(output_file) {
            Ok(file) => {
                let reader = std::io::BufReader::new(file);
                let historical_data: HistoricalPriceData = serde_json::from_reader(reader)?;
                price_lookup_map = historical_data.prices.into_iter().map(|p| (p.time, p.usd)).collect();
                if args.verbose {
                    println!("DEBUG: Price data loaded from existing prices.json ({} entries).", price_lookup_map.len());
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if args.verbose {
                    println!("DEBUG: prices.json not found. Attempting to fetch full historical prices.");
                }
                println!("prices.json not found. Attempting to fetch full historical prices...");
                price_lookup_map = fetch_full_historical_prices(&args).await?;
                let mut historical_data = HistoricalPriceData { prices: price_lookup_map.iter().map(|(&time, &usd)| PriceData { time, usd }).collect() };
                historical_data.prices.sort_by_key(|p| p.time); // Sort by timestamp
                let mut file = std::fs::File::create(output_file)?;
                serde_json::to_writer_pretty(&mut file, &historical_data)?;
                if args.verbose {
                    println!("Full historical prices saved to: {}. Loaded {} entries.", output_file, price_lookup_map.len());
                }
            },
            Err(e) => return Err(anyhow::anyhow!("Error opening prices.json: {}", e)),
        }
    }


    let other_pool_slugs: Vec<String> = if let Some(pools_str) = args.other_pools.as_ref() {
        pools_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else {
        if args.verbose {
            println!("No --other-pools specified. Attempting to fetch active pools from mempool.space...");
        }
        let fetched_pools = fetch_mempool_pools("1y", args.depth, &args).await?;
        if fetched_pools.is_empty() {
            eprintln!("Error: Could not fetch any pools from mempool.space. Please specify --other-pools manually.");
            std::process::exit(1);
        } else {
            if args.verbose {
                println!("Using fetched pools: {}", fetched_pools.join(", "));
            }
            fetched_pools
        }
    };

    let summary_results = compare_pool_losses(
        &args.ocean_slug,
        &other_pool_slugs,
        args.depth,
        &args,
        &price_lookup_map,
    ).await?;

    print_summary_table(&summary_results);

    Ok(())
}
