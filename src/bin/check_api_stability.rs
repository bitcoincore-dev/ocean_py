use anyhow::{Result, Context};
use tokio::time::{sleep, Duration};
use ocean_loss_estimator_rs::fetch_blocks_sample;
#[tokio::main]
async fn main() -> Result<()> {
    println!("--- Checking API Stability for Ocean Mining Pool Blocks ---");

    println!("Fetching sample 1...");
    let sample1 = fetch_blocks_sample(5).await.context("Failed to fetch sample 1")?;

    sleep(Duration::from_secs(2)).await;

    println!("Fetching sample 2...");
    let sample2 = fetch_blocks_sample(5).await.context("Failed to fetch sample 2")?;

    if sample1.len() != sample2.len() {
        eprintln!("Error: Sample lengths differ. Cannot compare.");
        return Ok(());
    }

    let mut differences_found = false;
    for i in 0..sample1.len() {
        let block1 = &sample1[i];
        let block2 = &sample2[i];

        let height = block1.height;
        let id_val = &block1.id;

        // Compare matchRate
        let match_rate1 = block1.extras.as_ref().and_then(|e| e.match_rate);
        let match_rate2 = block2.extras.as_ref().and_then(|e| e.match_rate);
        if match_rate1 != match_rate2 {
            println!("Difference in matchRate for Block {} ({}): Sample 1={:?}, Sample 2={:?}",
                     height, id_val, match_rate1, match_rate2);
            differences_found = true;
        }
        
        // Compare reward
        let reward1 = block1.extras.as_ref().and_then(|e| e.reward);
        let reward2 = block2.extras.as_ref().and_then(|e| e.reward);
        if reward1 != reward2 {
            println!("Difference in reward for Block {} ({}): Sample 1={:?}, Sample 2={:?}",
                     height, id_val, reward1, reward2);
            differences_found = true;
        }
            
        // Compare expectedFees
        let expected_fees1 = block1.extras.as_ref().and_then(|e| e.expected_fees);
        let expected_fees2 = block2.extras.as_ref().and_then(|e| e.expected_fees);
        if expected_fees1 != expected_fees2 {
            println!("Difference in expectedFees for Block {} ({}): Sample 1={:?}, Sample 2={:?}",
                     height, id_val, expected_fees1, expected_fees2);
            differences_found = true;
        }
    }

    if !differences_found {
        println!("
No differences found in matchRate, reward, or expectedFees for the sampled blocks.");
    } else {
        println!("
Differences found in sampled blocks. API data may not be perfectly static for these fields.");
    }

    Ok(())
}
