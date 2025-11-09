use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;
use std::io::Error;

// Ore program structures based on analysis of regolith-labs/ore source code

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Board {
    /// The current round number
    pub round_id: u64,

    /// The slot at which the current round starts mining
    pub start_slot: u64,

    /// The slot at which the current round ends mining
    pub end_slot: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Round {
    /// The round number
    pub id: u64,

    /// The amount of SOL deployed in each square
    pub deployed: [u64; 25],

    /// The hash of the end slot, provided by solana, used for random number generation
    pub slot_hash: [u8; 32],

    /// The count of miners on each square
    pub count: [u64; 25],

    /// The slot at which claims for this round account end
    pub expires_at: u64,

    /// The amount of ORE in the motherlode
    pub motherlode: u64,

    /// The account to which rent should be returned when this account is closed
    pub rent_payer: Pubkey,

    /// The top miner of the round
    pub top_miner: Pubkey,

    /// The amount of ORE to distribute to the top miner
    pub top_miner_reward: u64,

    /// The total amount of SOL deployed in the round
    pub total_deployed: u64,

    /// The total amount of SOL put in the ORE vault
    pub total_vaulted: u64,

    /// The total amount of SOL won by miners for the round
    pub total_winnings: u64,
}

impl Round {
    /// Get RNG value from slot hash
    pub fn rng(&self) -> Option<u64> {
        if self.slot_hash == [0; 32] || self.slot_hash == [u8::MAX; 32] {
            return None;
        }
        let r1 = u64::from_le_bytes(self.slot_hash[0..8].try_into().unwrap());
        let r2 = u64::from_le_bytes(self.slot_hash[8..16].try_into().unwrap());
        let r3 = u64::from_le_bytes(self.slot_hash[16..24].try_into().unwrap());
        let r4 = u64::from_le_bytes(self.slot_hash[24..32].try_into().unwrap());
        let r = r1 ^ r2 ^ r3 ^ r4;
        Some(r)
    }

    /// Get the winning square index (0-24)
    pub fn winning_square(&self, rng: u64) -> usize {
        (rng % 25) as usize
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Miner {
    /// The authority of this miner account
    pub authority: Pubkey,

    /// The miner's prospects in the current round
    pub deployed: [u64; 25],

    /// The cumulative amount of SOL deployed on each square prior to this miner's move
    pub cumulative: [u64; 25],

    /// SOL withheld in reserve to pay for checkpointing
    pub checkpoint_fee: u64,

    /// The last round that this miner checkpointed
    pub checkpoint_id: u64,

    /// The last time this miner claimed ORE rewards
    pub last_claim_ore_at: i64,

    /// The last time this miner claimed SOL rewards
    pub last_claim_sol_at: i64,

    /// The rewards factor last time rewards were updated on this miner account
    pub rewards_factor: [u8; 16], // Numeric type (u128)

    /// The amount of SOL this miner can claim
    pub rewards_sol: u64,

    /// The amount of ORE this miner can claim
    pub rewards_ore: u64,

    /// The amount of ORE this miner has earned from claim fees
    pub refined_ore: u64,

    /// The ID of the round this miner last played in
    pub round_id: u64,

    /// The total amount of SOL this miner has mined across all blocks
    pub lifetime_rewards_sol: u64,

    /// The total amount of ORE this miner has mined across all blocks
    pub lifetime_rewards_ore: u64,
}

// Manual Pod/Zeroable implementation for Miner (contains Pubkey)
unsafe impl Pod for Miner {}
unsafe impl Zeroable for Miner {}

/// Helper for deserializing account data with 8-byte discriminator
pub fn deserialize_account<T: Pod>(data: &[u8]) -> Result<&T, Error> {
    if data.len() < 8 + std::mem::size_of::<T>() {
        return Err(Error::new(
            std::io::ErrorKind::InvalidData,
            "Account data too short"
        ));
    }

    // Skip 8-byte discriminator
    let account_data = &data[8..];
    bytemuck::try_from_bytes(account_data)
        .map_err(|_| Error::new(
            std::io::ErrorKind::InvalidData,
            "Failed to deserialize account"
        ))
}
