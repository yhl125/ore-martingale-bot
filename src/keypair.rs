use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::signer::keypair::keypair_from_seed;
use anyhow::{Context, Result};

/// Load keypair from Base58-encoded private key string
/// Example: "4YFq9y5f5hi77Bq8kDCE6VgqoAqKGSQN87yW9YeGybpNfqKUG4WxnwhboHGUeXjY7g8262mhL1kCCM9yy8uGvdj7"
pub fn load_keypair(private_key_base58: &str) -> Result<Keypair> {
    // Decode Base58 string to bytes
    let keypair_bytes = bs58::decode(private_key_base58)
        .into_vec()
        .context("Failed to decode Base58 private key")?;

    // Solana private key contains 64 bytes (32-byte seed + 32-byte public key)
    if keypair_bytes.len() != 64 {
        anyhow::bail!("Invalid private key: expected 64 bytes, got {}", keypair_bytes.len());
    }

    // Extract the first 32 bytes as the seed
    let seed: [u8; 32] = keypair_bytes[0..32]
        .try_into()
        .context("Failed to extract seed from private key")?;

    // Create keypair from seed
    let keypair = keypair_from_seed(&seed)
        .map_err(|e| anyhow::anyhow!("Failed to create keypair: {}", e))?;

    log::info!("Loaded keypair: {}", keypair.pubkey());
    Ok(keypair)
}
