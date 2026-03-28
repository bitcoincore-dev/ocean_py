use anyhow::Result;
use ocean_loss_estimator_rs::fetch_and_save_pool_data;

#[tokio::main]
async fn main() -> Result<()> {
    fetch_and_save_pool_data().await
}
