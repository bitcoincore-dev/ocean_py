use anyhow::{Result, Context};
use tokio::io::AsyncWriteExt;
use ocean_loss_estimator_rs::{Pool, fetch_and_save_pool_data};

#[tokio::main]
async fn main() -> Result<()> {
    fetch_and_save_pool_data().await
}
