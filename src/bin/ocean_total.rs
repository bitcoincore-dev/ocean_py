use ocean_loss_estimator_rs::{fetch_all_ocean_blocks_rust, Args};
use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    fetch_all_ocean_blocks_rust(args.depth).await
}
