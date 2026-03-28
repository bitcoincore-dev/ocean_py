use anyhow::Result;
use ocean_loss_estimator_rs::utils::fetch_and_save_full_historical_prices;

#[tokio::main]
async fn main() -> Result<()> {
    fetch_and_save_full_historical_prices().await
}
