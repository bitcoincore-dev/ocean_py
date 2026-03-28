use anyhow::Result;
use ocean_loss_estimator_rs::fetch_concurrent_ocean_report_rust;

#[tokio::main]
async fn main() -> Result<()> {
    fetch_concurrent_ocean_report_rust().await
}
