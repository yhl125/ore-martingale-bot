use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

// Ore program constants from regolith-labs/ore source code
// https://github.com/regolith-labs/ore/blob/main/api/src/lib.rs
// Mainnet program ID: declare_id!("oreV3EG1i9BEgiAJ8b177Z2S2rMarzak4NMv1kULvWv");

pub const ORE_PROGRAM_ID: &str = "oreV3EG1i9BEgiAJ8b177Z2S2rMarzak4NMv1kULvWv";

// PDA seeds
pub const BOARD: &[u8] = b"board";
pub const ROUND: &[u8] = b"round";
pub const MINER: &[u8] = b"miner";
pub const AUTOMATION: &[u8] = b"automation";

/// Get the Board PDA
pub fn get_board_pda() -> (Pubkey, u8) {
    let program_id = Pubkey::from_str(ORE_PROGRAM_ID).unwrap();
    Pubkey::find_program_address(&[BOARD], &program_id)
}

/// Get the Round PDA for a specific round ID
pub fn get_round_pda(round_id: u64) -> (Pubkey, u8) {
    let program_id = Pubkey::from_str(ORE_PROGRAM_ID).unwrap();
    Pubkey::find_program_address(
        &[ROUND, &round_id.to_le_bytes()],
        &program_id
    )
}

/// Get the Miner PDA for an authority
pub fn get_miner_pda(authority: &Pubkey) -> (Pubkey, u8) {
    let program_id = Pubkey::from_str(ORE_PROGRAM_ID).unwrap();
    Pubkey::find_program_address(&[MINER, authority.as_ref()], &program_id)
}

/// Get the Automation PDA for an authority
pub fn get_automation_pda(authority: &Pubkey) -> (Pubkey, u8) {
    let program_id = Pubkey::from_str(ORE_PROGRAM_ID).unwrap();
    Pubkey::find_program_address(&[AUTOMATION, authority.as_ref()], &program_id)
}

/// Get the Treasury PDA
pub fn get_treasury_pda() -> (Pubkey, u8) {
    let program_id = Pubkey::from_str(ORE_PROGRAM_ID).unwrap();
    Pubkey::find_program_address(&[b"treasury"], &program_id)
}

/// Get the Ore program ID as a Pubkey
pub fn ore_program_id() -> Pubkey {
    Pubkey::from_str(ORE_PROGRAM_ID).unwrap()
}
