use bytemuck::{Pod, Zeroable};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use crate::ore::pda::{get_automation_pda, get_board_pda, get_miner_pda, get_round_pda, get_treasury_pda, ore_program_id};

// System program ID constant
pub const SYSTEM_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("11111111111111111111111111111111");

/// Deploy instruction data
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DeployData {
    pub amount: [u8; 8],   // u64 in little-endian
    pub squares: [u8; 4],  // u32 mask in little-endian
}

/// Instruction discriminators (from Ore source code)
pub const DEPLOY_DISCRIMINATOR: u8 = 6;

/// Build a Deploy instruction
///
/// Deploys capital to prospect on squares.
///
/// # Arguments
/// * `signer` - The account paying for the transaction
/// * `authority` - The miner authority (usually same as signer)
/// * `amount` - Amount of lamports to deploy per square
/// * `round_id` - The current round ID
/// * `squares` - Array of 25 booleans indicating which squares to bet on
pub fn build_deploy_instruction(
    signer: Pubkey,
    authority: Pubkey,
    amount: u64,
    round_id: u64,
    squares: [bool; 25],
) -> Instruction {
    // Convert boolean array to 32-bit mask
    let mut mask: u32 = 0;
    for (i, &should_deploy) in squares.iter().enumerate() {
        if should_deploy {
            mask |= 1 << i;
        }
    }

    // Derive PDAs
    let automation_address = get_automation_pda(&authority).0;
    let board_address = get_board_pda().0;
    let miner_address = get_miner_pda(&authority).0;
    let round_address = get_round_pda(round_id).0;

    // Create instruction data
    let deploy_data = DeployData {
        amount: amount.to_le_bytes(),
        squares: mask.to_le_bytes(),
    };

    // Serialize instruction: discriminator + data
    let mut instruction_data = vec![DEPLOY_DISCRIMINATOR];
    instruction_data.extend_from_slice(bytemuck::bytes_of(&deploy_data));

    Instruction {
        program_id: ore_program_id(),
        accounts: vec![
            AccountMeta::new(signer, true),                        // Signer
            AccountMeta::new(authority, false),                    // Authority
            AccountMeta::new(automation_address, false),           // Automation (may be empty)
            AccountMeta::new(board_address, false),                // Board
            AccountMeta::new(miner_address, false),                // Miner
            AccountMeta::new(round_address, false),                // Round
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),  // System program
            // Note: Entropy accounts omitted for simplicity
            // In production, these would be required for VRF
        ],
        data: instruction_data,
    }
}

/// Checkpoint instruction to claim rewards after a round completes
pub fn build_checkpoint_instruction(
    signer: Pubkey,
    miner_authority: Pubkey,
    miner_round_id: u64,
) -> Instruction {
    const CHECKPOINT_DISCRIMINATOR: u8 = 2;

    let board_address = get_board_pda().0;
    let miner_address = get_miner_pda(&miner_authority).0;
    let round_address = get_round_pda(miner_round_id).0;
    let treasury_address = get_treasury_pda().0;

    Instruction {
        program_id: ore_program_id(),
        accounts: vec![
            AccountMeta::new(signer, true),              // signer
            AccountMeta::new(board_address, false),      // board
            AccountMeta::new(miner_address, false),      // miner
            AccountMeta::new(round_address, false),      // round
            AccountMeta::new(treasury_address, false),   // treasury
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false), // system_program
        ],
        data: vec![CHECKPOINT_DISCRIMINATOR],
    }
}

/// Claim SOL rewards
pub fn build_claim_sol_instruction(signer: Pubkey) -> Instruction {
    const CLAIM_SOL_DISCRIMINATOR: u8 = 3;

    let miner_address = get_miner_pda(&signer).0;

    Instruction {
        program_id: ore_program_id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(miner_address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: vec![CLAIM_SOL_DISCRIMINATOR],
    }
}
