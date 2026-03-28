#[cfg(test)]
mod tests {
    use super::super::utils::fetch_block_transactions_rust;
    use super::super::models::CoinbaseInfo;
    use anyhow::Result;
    use super::super::utils::fetch_from_mirror;
    use serde_json::Value;

    #[tokio::test]
    async fn test_fetch_block_transactions_miner_detection() -> Result<()> {
        // This is the hash of a known block mined by Peak Mining (Block 942655)
        let block_hash = "00000000000000000001d5b2cbf42dc9fd34a83e34f88c350357283a222cc2aa";

        let coinbase_info: CoinbaseInfo = fetch_block_transactions_rust(block_hash).await?;

        dbg!(&coinbase_info);

        assert_eq!(coinbase_info.miner_name, Some("Peak Mining".to_string()));
        assert!(!coinbase_info.op_return_data.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_from_mirror_success() -> Result<()> {
        // Use a known stable endpoint that returns a simple value
        let path = "/api/v1/blocks/tip/height";
        let response: Value = fetch_from_mirror(path, 0, 10).await?;

        dbg!(&response);

        // Assert that the response is a number (block height) and is greater than 0
        let block_height = response.as_u64().context("Response is not a u64")?;
        assert!(block_height > 0, "Block height should be greater than 0");

        Ok(())
    }
}
