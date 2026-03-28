#[cfg(test)]
mod tests {
    use super::super::utils::fetch_block_transactions_rust;
    use super::super::models::CoinbaseInfo;
    use anyhow::Result;

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
}
