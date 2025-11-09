use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use std::fs::read_to_string;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BotConfig {
    pub rpc_url: String,
    pub private_key: String,
    pub martingale: MartingaleConfig,
    pub monitoring: MonitoringConfig,
    pub discord: DiscordConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MartingaleConfig {
    pub base_bet_amount: f64,         // Initial bet in SOL (e.g., 0.01)
    pub max_consecutive_losses: u8,   // Max losses before reset (bet doubles each loss)
    pub warn_consecutive_losses: u8,  // Send Discord warning at this loss count
    pub blocks_per_bet: u8,           // Number of grid blocks to bet on (1-25)
}

impl MartingaleConfig {
    /// Convert SOL amount to lamports
    pub fn base_bet_lamports(&self) -> u64 {
        (self.base_bet_amount * 1_000_000_000.0) as u64
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MonitoringConfig {
    pub min_balance_sol: f64,         // Minimum balance in SOL (emergency stop threshold)
    #[serde(default = "default_auto_claim_threshold")]
    pub auto_claim_sol_threshold: f64, // Auto-claim SOL when rewards >= this (default: 0.1 SOL)
}

impl MonitoringConfig {
    /// Convert min_balance_sol to lamports
    pub fn min_balance_lamports(&self) -> u64 {
        (self.min_balance_sol * 1_000_000_000.0) as u64
    }

    /// Convert auto_claim_sol_threshold to lamports
    pub fn auto_claim_sol_threshold_lamports(&self) -> u64 {
        (self.auto_claim_sol_threshold * 1_000_000_000.0) as u64
    }
}

fn default_auto_claim_threshold() -> f64 {
    0.1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiscordConfig {
    pub webhook_url: String,
    pub stats_webhook_url: String,
    pub warn_webhook_url: String,
    #[serde(default = "default_stats_interval")]
    pub stats_notification_interval: u32,
}

fn default_stats_interval() -> u32 {
    10
}

pub fn load_config(path: &str) -> Result<BotConfig> {
    let config_str = read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path))?;

    let config: BotConfig = serde_json::from_str(&config_str)
        .context("Failed to parse config JSON")?;

    // Validate config
    if config.martingale.blocks_per_bet == 0 || config.martingale.blocks_per_bet > 25 {
        anyhow::bail!("blocks_per_bet must be between 1 and 25");
    }

    if config.martingale.warn_consecutive_losses > config.martingale.max_consecutive_losses {
        anyhow::bail!("warn_consecutive_losses must be <= max_consecutive_losses");
    }

    log::info!("Loaded config from: {}", path);
    log::info!("  RPC URL: {}", config.rpc_url);
    log::info!("  Base bet: {} SOL", config.martingale.base_bet_amount);
    log::info!("  Max consecutive losses: {}", config.martingale.max_consecutive_losses);
    log::info!("  Blocks per bet: {}", config.martingale.blocks_per_bet);

    Ok(config)
}
