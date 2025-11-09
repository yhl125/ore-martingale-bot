use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_sdk::pubkey::Pubkey;
use anyhow::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct SolanaClient {
    pub rpc: Arc<RpcClient>,
}

impl SolanaClient {
    pub async fn new(rpc_url: &str) -> Result<Self> {
        let rpc = RpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig { commitment: CommitmentLevel::Confirmed },
        );

        // Test connection
        let block_height = rpc.get_block_height().await?;
        log::info!("Connected to Solana cluster. Block height: {}", block_height);

        Ok(Self { rpc: Arc::new(rpc) })
    }

    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        let balance = self.rpc.get_balance(pubkey).await?;
        Ok(balance)
    }
}
