use anyhow::Result;
use ocean_loss_estimator_rs::{
    fetch_ocean_data_rust,
    generate_ocean_config_env_rust,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Configure the local environment
    generate_ocean_config_env_rust();

    // 2. Fetch live data from the API
    fetch_ocean_data_rust().await
}
