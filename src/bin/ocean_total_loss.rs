use anyhow::Result;
use ocean_loss_estimator_rs::fetch_total_loss_ocean_report_rust;

#[tokio::main]
async fn main() -> Result<()> {
    fetch_total_loss_ocean_report_rust().await
}
