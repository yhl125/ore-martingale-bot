pub mod instruction;
pub mod pda;
pub mod state;

use crate::client::SolanaClient;
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use state::{Board, Miner, Round, deserialize_account};

#[derive(Clone)]
pub struct OreClient {
    pub solana: SolanaClient,
}

impl OreClient {
    pub fn new(solana: SolanaClient) -> Self {
        Self { solana }
    }

    /// Get the Board account
    pub async fn get_board(&self) -> Result<Board> {
        let (board_address, _bump) = pda::get_board_pda();
        let account_data = self.solana.rpc.get_account_data(&board_address).await?;
        let board = deserialize_account::<Board>(&account_data)?;
        Ok(*board)
    }

    /// Get a Round account by ID
    pub async fn get_round(&self, round_id: u64) -> Result<Round> {
        let (round_address, _bump) = pda::get_round_pda(round_id);
        let account_data = self.solana.rpc.get_account_data(&round_address).await?;
        let round = deserialize_account::<Round>(&account_data)?;
        Ok(*round)
    }

    /// Get a Miner account by authority
    pub async fn get_miner(&self, authority: &Pubkey) -> Result<Option<Miner>> {
        let (miner_address, _bump) = pda::get_miner_pda(authority);

        match self.solana.rpc.get_account_data(&miner_address).await {
            Ok(account_data) => {
                let miner = deserialize_account::<Miner>(&account_data)?;
                Ok(Some(*miner))
            }
            Err(_) => Ok(None), // Miner account doesn't exist yet
        }
    }

    /// Check if a round is active (within start and end slots)
    pub async fn is_round_active(&self, board: &Board) -> Result<bool> {
        let slot = self.solana.rpc.get_slot().await?;
        Ok(slot >= board.start_slot && slot < board.end_slot)
    }

    /// Check if a round has ended and slot_hash is available
    pub async fn is_round_complete(&self, board: &Board) -> Result<bool> {
        let slot = self.solana.rpc.get_slot().await?;
        Ok(slot >= board.end_slot)
    }

    /// Get the Miner PDA address for a given authority
    pub fn get_miner_pda(&self, authority: &Pubkey) -> Pubkey {
        pda::get_miner_pda(authority).0
    }
}
