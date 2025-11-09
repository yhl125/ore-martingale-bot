use anyhow::Result;
use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair,
    transaction::Transaction,
};
use crate::client::SolanaClient;
use crate::mining::grid::BlockPosition;
use crate::ore::instruction::{build_deploy_instruction, build_claim_sol_instruction, build_checkpoint_instruction};

#[derive(Clone)]
pub struct TransactionExecutor {
    solana: SolanaClient,
    max_retries: u8,
}

impl TransactionExecutor {
    pub fn new(solana: SolanaClient, max_retries: u8) -> Self {
        Self {
            solana,
            max_retries,
        }
    }

    /// Execute bet transaction with retry logic
    pub async fn execute_bet(
        &self,
        signer: &dyn Signer,
        round_id: u64,
        blocks: &[BlockPosition],
        bet_per_block: u64,
    ) -> Result<String> {
        // Convert BlockPosition to boolean array
        let mut squares = [false; 25];
        for block in blocks {
            squares[block.index as usize] = true;
        }

        // Build deploy instruction
        let instruction = build_deploy_instruction(
            signer.pubkey(),
            signer.pubkey(), // Authority is same as signer
            bet_per_block,
            round_id,
            squares,
        );

        log::debug!("🔨 Building Deploy instruction for {} blocks", blocks.len());
        for block in blocks {
            log::debug!("   - Block {} (row: {}, col: {})", block.index, block.row, block.col);
        }

        self.send_transaction_with_retry(signer, vec![instruction]).await
    }

    /// Execute checkpoint + bet in single transaction
    pub async fn execute_checkpoint_and_bet(
        &self,
        signer: &dyn Signer,
        miner_round_id: u64,
        bet_round_id: u64,
        blocks: &[BlockPosition],
        bet_per_block: u64,
    ) -> Result<String> {
        // Convert BlockPosition to boolean array
        let mut squares = [false; 25];
        for block in blocks {
            squares[block.index as usize] = true;
        }

        // Build checkpoint instruction
        let checkpoint_ix = build_checkpoint_instruction(
            signer.pubkey(),
            signer.pubkey(),
            miner_round_id,
        );

        // Build deploy instruction
        let deploy_ix = build_deploy_instruction(
            signer.pubkey(),
            signer.pubkey(),
            bet_per_block,
            bet_round_id,
            squares,
        );

        log::debug!("🔨 Building combined Checkpoint + Deploy transaction");
        log::debug!("   Checkpoint: round #{}", miner_round_id);
        log::debug!("   Deploy: {} blocks on round #{}", blocks.len(), bet_round_id);
        for block in blocks {
            log::debug!("   - Block {} (row: {}, col: {})", block.index, block.row, block.col);
        }

        // Send both instructions in single transaction
        self.send_transaction_with_retry(signer, vec![checkpoint_ix, deploy_ix]).await
    }

    /// Execute claim SOL transaction (takes owned Keypair for Send + 'static compatibility)
    pub async fn execute_claim_sol(
        &self,
        signer: Keypair,
    ) -> Result<String> {
        // Build claim SOL instruction
        let instruction = build_claim_sol_instruction(signer.pubkey());

        log::debug!("🔨 Building Claim SOL instruction");

        self.send_transaction_with_retry_keypair(signer, vec![instruction]).await
    }

    /// Send transaction with retry logic (for Keypair)
    async fn send_transaction_with_retry_keypair(
        &self,
        signer: Keypair,
        instructions: Vec<solana_sdk::instruction::Instruction>,
    ) -> Result<String> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.send_transaction_keypair(&signer, &instructions).await {
                Ok(signature) => {
                    log::info!("✅ Transaction confirmed: {}", signature);
                    return Ok(signature);
                }
                Err(e) => {
                    log::warn!("❌ Transaction attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        // Exponential backoff
                        let delay = std::time::Duration::from_millis(100 * (2_u64.pow(attempt as u32)));
                        log::info!("⏳ Retrying in {:?}...", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Transaction failed after {} retries", self.max_retries)))
    }

    /// Send transaction and wait for confirmation (for Keypair)
    async fn send_transaction_keypair(
        &self,
        signer: &Keypair,
        instructions: &[solana_sdk::instruction::Instruction],
    ) -> Result<String> {
        // Get recent blockhash
        let recent_blockhash = self.solana.rpc.get_latest_blockhash().await?;

        // Create and sign transaction
        let mut transaction = Transaction::new_with_payer(instructions, Some(&signer.pubkey()));
        transaction.sign(&[signer], recent_blockhash);

        // Send and confirm transaction
        let signature = self.solana.rpc
            .send_and_confirm_transaction(&transaction)
            .await?;

        Ok(signature.to_string())
    }

    /// Send transaction with retry logic
    async fn send_transaction_with_retry(
        &self,
        signer: &dyn Signer,
        instructions: Vec<solana_sdk::instruction::Instruction>,
    ) -> Result<String> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.send_transaction(signer, &instructions).await {
                Ok(signature) => {
                    log::info!("✅ Transaction confirmed: {}", signature);
                    return Ok(signature);
                }
                Err(e) => {
                    log::warn!("❌ Transaction attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        // Exponential backoff
                        let delay = std::time::Duration::from_millis(100 * (2_u64.pow(attempt as u32)));
                        log::info!("⏳ Retrying in {:?}...", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Transaction failed after {} retries", self.max_retries)))
    }

    /// Send transaction and wait for confirmation
    async fn send_transaction(
        &self,
        signer: &dyn Signer,
        instructions: &[solana_sdk::instruction::Instruction],
    ) -> Result<String> {
        // Get recent blockhash
        let recent_blockhash = self.solana.rpc.get_latest_blockhash().await?;

        // Create and sign transaction
        let mut transaction = Transaction::new_with_payer(instructions, Some(&signer.pubkey()));
        transaction.sign(&[signer], recent_blockhash);

        // Send and confirm transaction
        let signature = self.solana.rpc
            .send_and_confirm_transaction(&transaction)
            .await?;

        Ok(signature.to_string())
    }
}
