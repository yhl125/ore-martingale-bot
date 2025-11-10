mod client;
mod config;
mod discord;
mod keypair;
mod mining;
mod ore;
mod subscription;

use anyhow::Result;
use client::SolanaClient;
use config::load_config;
use discord::DiscordNotifier;
use keypair::load_keypair;
use mining::executor::TransactionExecutor;
use mining::grid;
use mining::strategy::MartingaleState;
use ore::OreClient;
use solana_sdk::signature::Signer;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use subscription::MinerSubscription;
use tokio::time::sleep;

// Application-wide constants
const SOLANA_SLOT_TIME_SECONDS: f64 = 0.4; // ~400ms per slot
const ROUND_START_BUFFER_SECONDS: u64 = 2; // Buffer before round starts
const ROUND_COMPLETION_POLL_INTERVAL_SECS: u64 = 10; // Polling interval for round completion
const ROUND_COMPLETION_TIMEOUT_SECS: u64 = 120; // 2 minute timeout
const RNG_RETRY_INTERVAL_SECS: u64 = 2; // Retry interval for RNG availability
const MAX_RNG_ATTEMPTS: u8 = 20; // Max attempts to get RNG
const REWARDS_RETRY_INTERVAL_SECS: u64 = 2; // Retry interval for rewards update
const MAX_REWARDS_RETRIES: u8 = 10; // Max retries for rewards update
const WSS_UPDATE_TIMEOUT_SECS: u64 = 3; // WebSocket update timeout
const MAX_TX_RETRIES: u8 = 3; // Max transaction retry attempts
const DEFAULT_NEXT_ROUND_WAIT_SECS: u64 = 5; // Default wait time for next round
const ERROR_RETRY_WAIT_SECS: u64 = 10; // Wait time before retry on error
const RPC_ERROR_WAIT_SECS: u64 = 10; // Wait time on RPC error

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    log::info!("🚀 Ore Martingale Bot starting...");

    // Load configuration
    let config = load_config("config.json")?;

    // Initialize Solana client
    let solana_client = SolanaClient::new(&config.rpc_url).await?;
    log::info!("✅ Connected to Solana RPC");

    // Load keypair
    let signer = load_keypair(&config.private_key)?;
    log::info!("✅ Loaded keypair: {}", signer.pubkey());

    // Check balance
    let balance = solana_client.get_balance(&signer.pubkey()).await?;
    log::info!("💰 Balance: {:.6} SOL", balance as f64 / 1e9);

    if balance < config.monitoring.min_balance_lamports() {
        anyhow::bail!(
            "⚠️ Balance ({:.6} SOL) is below minimum threshold ({:.6} SOL). Please top up.",
            balance as f64 / 1e9,
            config.monitoring.min_balance_sol
        );
    }

    // Initialize Ore client
    let ore_client = OreClient::new(solana_client.clone());
    log::info!("✅ Ore client initialized");

    // Initialize Discord notifier
    let discord = DiscordNotifier::new(
        config.discord.webhook_url.clone(),
        config.discord.stats_webhook_url.clone(),
        config.discord.warn_webhook_url.clone(),
    );
    log::info!("✅ Discord notifier initialized");

    // Initialize transaction executor
    let executor = TransactionExecutor::new(solana_client.clone(), MAX_TX_RETRIES);
    log::info!("✅ Transaction executor initialized (max retries: {})", MAX_TX_RETRIES);

    log::info!("✅ Grid selector initialized (random selection)");

    // Initialize martingale state (wrapped in Arc<Mutex> for sharing with async tasks)
    let martingale_state = Arc::new(Mutex::new(MartingaleState::new(config.martingale.base_bet_lamports())));

    // Check initial rewards from miner account (if exists)
    if let Some(miner) = ore_client.get_miner(&signer.pubkey()).await? {
        log::info!("💰 Existing unclaimed rewards: {:.6} SOL", miner.rewards_sol as f64 / 1e9);
    }

    log::info!("✅ Martingale state initialized");
    log::info!("   Base bet: {:.6} SOL per block", config.martingale.base_bet_amount);
    log::info!("   Max consecutive losses: {}", config.martingale.max_consecutive_losses);
    log::info!("   Warning threshold: {}", config.martingale.warn_consecutive_losses);
    log::info!("   Blocks per bet: {}", config.martingale.blocks_per_bet);

    // Start WebSocket subscription for real-time miner updates
    let miner_pda = ore_client.get_miner_pda(&signer.pubkey());
    let subscription = MinerSubscription::new(config.rpc_url.clone(), miner_pda).await?;
    log::info!("📡 WebSocket subscription started");

    log::info!("🚀 Starting main betting loop...");

    // Main event loop
    loop {
        match run_betting_round(
            &ore_client,
            &executor,
            &martingale_state,
            &discord,
            &signer,
            &config,
            &subscription,
        ).await {
            Ok(should_continue) => {
                if !should_continue {
                    log::warn!("⚠️ Max consecutive losses reached. Pausing bot.");

                    // Send error notification
                    if let Err(e) = discord.notify_error("Max consecutive losses reached. Bot paused.").await {
                        log::error!("Failed to send Discord notification: {}", e);
                    }

                    break;
                }
            }
            Err(e) => {
                log::error!("❌ Error in betting round: {}", e);

                // Send error notification
                if let Err(e) = discord.notify_error(&format!("Error: {}", e)).await {
                    log::error!("Failed to send Discord notification: {}", e);
                }

                // Wait before retrying
                log::info!("⏳ Waiting {} seconds before retry...", ERROR_RETRY_WAIT_SECS);
                sleep(Duration::from_secs(ERROR_RETRY_WAIT_SECS)).await;
            }
        }

        // Check balance periodically
        let balance = solana_client.get_balance(&signer.pubkey()).await?;
        if balance < config.monitoring.min_balance_lamports() {
            log::error!("⚠️ Balance too low: {:.6} SOL", balance as f64 / 1e9);

            if let Err(e) = discord.notify_error(&format!(
                "Balance too low: {:.6} SOL. Please top up.",
                balance as f64 / 1e9
            )).await {
                log::error!("Failed to send Discord notification: {}", e);
            }

            break;
        }

        // Calculate dynamic wait time until next round
        match ore_client.get_board().await {
            Ok(current_board) => {
                match ore_client.solana.rpc.get_slot().await {
                    Ok(current_slot) => {
                        if current_slot < current_board.start_slot {
                            // Next round hasn't started yet
                            let slots_until_start = current_board.start_slot - current_slot;
                            let seconds_until_start = (slots_until_start as f64 * SOLANA_SLOT_TIME_SECONDS) as u64;
                            let wait_time = seconds_until_start + ROUND_START_BUFFER_SECONDS;
                            log::info!("⏳ Next round starts in ~{} seconds (slot {} -> {})",
                                wait_time, current_slot, current_board.start_slot);
                            sleep(Duration::from_secs(wait_time)).await;
                        } else {
                            // Already past start, wait default time
                            log::info!("⏳ Waiting for next round ({} seconds)...", DEFAULT_NEXT_ROUND_WAIT_SECS);
                            sleep(Duration::from_secs(DEFAULT_NEXT_ROUND_WAIT_SECS)).await;
                        }
                    }
                    Err(e) => {
                        log::warn!("⚠️ Failed to get current slot: {}. Waiting {} seconds...", e, RPC_ERROR_WAIT_SECS);
                        sleep(Duration::from_secs(RPC_ERROR_WAIT_SECS)).await;
                    }
                }
            }
            Err(e) => {
                log::warn!("⚠️ Failed to get board: {}. Waiting {} seconds...", e, RPC_ERROR_WAIT_SECS);
                sleep(Duration::from_secs(RPC_ERROR_WAIT_SECS)).await;
            }
        }
    }

    log::info!("👋 Bot shutting down gracefully");
    Ok(())
}

async fn run_betting_round(
    ore_client: &OreClient,
    executor: &TransactionExecutor,
    martingale_state: &Arc<Mutex<MartingaleState>>,
    discord: &DiscordNotifier,
    signer: &dyn Signer,
    config: &config::BotConfig,
    subscription: &MinerSubscription,
) -> Result<bool> {
    // Get current board state
    let board = ore_client.get_board().await?;
    let round_id = board.round_id;

    // Check if this is a new round
    {
        let mut state = martingale_state.lock().unwrap();
        if state.current_round != round_id {
            log::info!("🆕 New round detected: #{}", round_id);
            state.current_round = round_id;
        } else {
            log::debug!("📍 Round #{} (continuing)", round_id);
        }
    }

    // Check if round is active
    if !ore_client.is_round_active(&board).await? {
        let current_slot = ore_client.solana.rpc.get_slot().await?;
        if current_slot < board.start_slot {
            let slots_until_start = board.start_slot - current_slot;
            let seconds_until_start = (slots_until_start as f64 * SOLANA_SLOT_TIME_SECONDS) as u64;
            log::debug!("⏸️ Round not active yet. Starting in ~{} seconds (slot {} -> {})",
                seconds_until_start, current_slot, board.start_slot);
        } else {
            log::debug!("⏸️ Round not active yet. Waiting...");
        }
        return Ok(true);
    }

    // Get current round data (for future use)
    let _round = ore_client.get_round(round_id).await?;

    // Save current rewards before betting
    let (rewards_sol_before, rewards_ore_before) = if let Some(miner) = ore_client.get_miner(&signer.pubkey()).await? {
        log::debug!("💰 Current rewards before bet: {:.6} SOL, {:.6} ORE",
            miner.rewards_sol as f64 / 1e9,
            miner.rewards_ore as f64 / 1e11);
        (miner.rewards_sol, miner.rewards_ore)
    } else {
        (0, 0)
    };

    // Select blocks to bet on
    let blocks = grid::select_blocks(config.martingale.blocks_per_bet);
    let block_indices: Vec<u8> = blocks.iter().map(|b| b.index).collect();

    let (bet_per_block, consecutive_losses) = {
        let state = martingale_state.lock().unwrap();
        (state.current_bet_per_block, state.consecutive_losses)
    };
    let total_bet = bet_per_block * (blocks.len() as u64);

    log::info!("🎲 Betting on blocks: {:?}", block_indices);
    log::info!("💰 Bet: {:.6} SOL per block, total: {:.6} SOL",
        bet_per_block as f64 / 1e9,
        total_bet as f64 / 1e9
    );

    // Send bet notification to Discord
    if let Err(e) = discord.notify_bet(
        round_id,
        &block_indices,
        bet_per_block,
        total_bet,
        consecutive_losses,
    ).await {
        log::error!("Failed to send Discord notification: {}", e);
    }

    // Check if miner needs checkpoint and execute in single transaction
    if let Some(miner) = ore_client.get_miner(&signer.pubkey()).await? {
        if miner.checkpoint_id != miner.round_id {
            // Checkpoint needed - combine with deploy in single transaction
            log::info!("📤 Sending combined Checkpoint + Deploy transaction...");
            match executor.execute_checkpoint_and_bet(
                signer,
                miner.round_id,
                round_id,
                &blocks,
                bet_per_block,
            ).await {
                Ok(signature) => {
                    log::info!("✅ Checkpoint + Bet placed successfully!");
                    log::info!("   Signature: {}", signature);
                    martingale_state.lock().unwrap().record_bet(total_bet);
                }
                Err(e) => {
                    log::error!("❌ Failed to place checkpoint + bet: {}", e);
                    return Err(e);
                }
            }
        } else {
            // Already checkpointed - just deploy
            log::info!("✅ Miner already checkpointed, sending Deploy only...");
            log::info!("📤 Sending Deploy transaction...");
            match executor.execute_bet(signer, round_id, &blocks, bet_per_block).await {
                Ok(signature) => {
                    log::info!("✅ Bet placed successfully!");
                    log::info!("   Signature: {}", signature);
                    martingale_state.lock().unwrap().record_bet(total_bet);
                }
                Err(e) => {
                    log::error!("❌ Failed to place bet: {}", e);
                    return Err(e);
                }
            }
        }
    } else {
        // No miner account yet (first bet) - just deploy
        log::info!("ℹ️ No miner account found (first bet), sending Deploy only...");
        log::info!("📤 Sending Deploy transaction...");
        match executor.execute_bet(signer, round_id, &blocks, bet_per_block).await {
            Ok(signature) => {
                log::info!("✅ Bet placed successfully!");
                log::info!("   Signature: {}", signature);
                martingale_state.lock().unwrap().record_bet(total_bet);
            }
            Err(e) => {
                log::error!("❌ Failed to place bet: {}", e);
                return Err(e);
            }
        }
    }

    // Wait for round to complete (max 2 minutes)
    log::debug!("⏳ Waiting for round #{} to complete...", round_id);
    let max_wait_time = Duration::from_secs(ROUND_COMPLETION_TIMEOUT_SECS);
    let start_time = std::time::Instant::now();

    loop {
        tokio::time::sleep(Duration::from_secs(ROUND_COMPLETION_POLL_INTERVAL_SECS)).await;

        // Check timeout
        if start_time.elapsed() > max_wait_time {
            log::error!("⏰ Timeout waiting for round to complete ({} seconds)", ROUND_COMPLETION_TIMEOUT_SECS);
            anyhow::bail!("Round completion timeout");
        }

        // Check round status with retry on RPC error
        match ore_client.get_board().await {
            Ok(board_check) => {
                if ore_client.is_round_complete(&board_check).await.unwrap_or(false) {
                    log::debug!("🏁 Round #{} completed!", round_id);
                    break;
                }
            }
            Err(e) => {
                log::warn!("⚠️ RPC error checking round status: {}. Retrying...", e);
                continue;
            }
        }
    }

    // Get final round results with retry for RNG
    log::debug!("📊 Fetching final round results...");
    let mut final_round = ore_client.get_round(round_id).await?;
    let mut rng_attempts = 0;

    // Retry if RNG not available (slot_hash might not be ready immediately)
    while final_round.rng().is_none() && rng_attempts < MAX_RNG_ATTEMPTS {
        rng_attempts += 1;
        log::debug!("⏳ RNG not available yet, retrying ({}/{})...", rng_attempts, MAX_RNG_ATTEMPTS);
        tokio::time::sleep(Duration::from_secs(RNG_RETRY_INTERVAL_SECS)).await;
        final_round = ore_client.get_round(round_id).await?;
    }

    // Determine winner
    if let Some(rng) = final_round.rng() {
        let winning_square = final_round.winning_square(rng);
        log::info!("🎯 Winning square: {}", winning_square);

        // Check if we won
        let won = block_indices.contains(&(winning_square as u8));

        if won {
            log::info!("✅ WE WON!");

            // Get cycle bet total before resetting martingale state
            let cycle_bet_total = {
                let state = martingale_state.lock().unwrap();
                state.current_cycle_bet_lamports
            };

            // Reset martingale state immediately (won, so back to base bet)
            martingale_state.lock().unwrap().reset_after_win(&config.martingale);

            // Clone all necessary values for the async task
            let subscription_clone = subscription.clone();
            let ore_client_clone = ore_client.clone();
            let discord_clone = discord.clone();
            let executor_clone = executor.clone();
            let signer_pubkey = signer.pubkey();
            let config_clone = config.clone();
            let final_round_deployed = final_round.deployed[winning_square];
            let bet_per_block_clone = bet_per_block;
            let private_key_clone = config.private_key.clone();
            let martingale_state_clone = Arc::clone(&martingale_state);
            let discord_stats_clone = discord.clone();
            let config_stats_clone = config.clone();

            // Process rewards fetch and notifications asynchronously (non-blocking)
            tokio::spawn(async move {
                // ore-app pattern: Try WebSocket first (fast), fallback to RPC
                log::debug!("⏳ Waiting for rewards update...");
                let (mut rewards_sol_after, mut rewards_ore_after) = if let Some(miner) = subscription_clone
                    .wait_for_wss_update(rewards_sol_before, Duration::from_secs(WSS_UPDATE_TIMEOUT_SECS))
                    .await
                {
                    log::debug!("✅ Rewards updated via WebSocket! {:.6} → {:.6} SOL",
                        rewards_sol_before as f64 / 1e9,
                        miner.rewards_sol as f64 / 1e9);
                    (miner.rewards_sol, miner.rewards_ore)
                } else {
                    // WebSocket didn't update quickly, fetch via RPC
                    log::debug!("📡 WebSocket timeout, fetching via RPC...");
                    if let Ok(Some(miner)) = ore_client_clone.get_miner(&signer_pubkey).await {
                        log::debug!("✅ Rewards fetched via RPC! {:.6} → {:.6} SOL",
                            rewards_sol_before as f64 / 1e9,
                            miner.rewards_sol as f64 / 1e9);
                        (miner.rewards_sol, miner.rewards_ore)
                    } else {
                        log::warn!("⚠️ Failed to fetch miner account");
                        (0, 0)
                    }
                };

                // Calculate actual rewards earned this round
                let mut sol_earned_actual = rewards_sol_after.saturating_sub(rewards_sol_before);
                let mut ore_earned_actual = rewards_ore_after.saturating_sub(rewards_ore_before);

                // If rewards haven't updated yet (equal or less than before), retry up to 10 times
                let mut retry_count = 0;
                while rewards_sol_after <= rewards_sol_before && retry_count < MAX_REWARDS_RETRIES {
                    retry_count += 1;
                    log::debug!("⚠️ Rewards not updated yet (before: {:.6}, after: {:.6}), retrying {}/{}...",
                        rewards_sol_before as f64 / 1e9,
                        rewards_sol_after as f64 / 1e9,
                        retry_count,
                        MAX_REWARDS_RETRIES);
                    tokio::time::sleep(Duration::from_secs(REWARDS_RETRY_INTERVAL_SECS)).await;

                    if let Ok(Some(miner)) = ore_client_clone.get_miner(&signer_pubkey).await {
                        rewards_sol_after = miner.rewards_sol;
                        rewards_ore_after = miner.rewards_ore;
                        sol_earned_actual = rewards_sol_after.saturating_sub(rewards_sol_before);
                        ore_earned_actual = rewards_ore_after.saturating_sub(rewards_ore_before);

                        if rewards_sol_after > rewards_sol_before {
                            log::debug!("✅ Rewards updated after {} retries: {:.6} SOL, {:.6} ORE",
                                retry_count,
                                sol_earned_actual as f64 / 1e9,
                                ore_earned_actual as f64 / 1e11);
                            break;
                        }
                    }
                }

                if rewards_sol_after <= rewards_sol_before {
                    log::warn!("⚠️ Rewards still not updated after {} retries (before: {:.6}, after: {:.6})",
                        retry_count,
                        rewards_sol_before as f64 / 1e9,
                        rewards_sol_after as f64 / 1e9);
                }

                log::info!("💰 Actual SOL earned (from protocol): {:.6} SOL", sol_earned_actual as f64 / 1e9);
                log::info!("📊 Total accumulated rewards: {:.6} SOL", rewards_sol_after as f64 / 1e9);
                log::info!("📊 Our bet: {:.6} SOL / Total on square: {:.6} SOL",
                    bet_per_block_clone as f64 / 1e9,
                    final_round_deployed as f64 / 1e9);

                // Check accumulated rewards for auto-claim
                let accumulated_rewards = if let Ok(Some(miner)) = ore_client_clone.get_miner(&signer_pubkey).await {
                    miner.rewards_sol
                } else {
                    0
                };

                // Auto-claim SOL if threshold reached
                let claim_threshold_lamports = config_clone.monitoring.auto_claim_sol_threshold_lamports();
                if accumulated_rewards >= claim_threshold_lamports {
                    log::info!("💰 SOL rewards reached threshold: {:.6} SOL >= {:.6} SOL",
                        accumulated_rewards as f64 / 1e9,
                        config_clone.monitoring.auto_claim_sol_threshold);
                    log::info!("📤 Executing claim SOL transaction...");

                    // Load keypair from private key
                    use crate::keypair::load_keypair;
                    match load_keypair(&private_key_clone) {
                        Ok(keypair) => {
                            match executor_clone.execute_claim_sol(keypair).await {
                                Ok(signature) => {
                                    log::info!("✅ SOL claimed successfully!");
                                    log::info!("   Signature: {}", signature);
                                    log::info!("   Amount: {:.6} SOL", accumulated_rewards as f64 / 1e9);

                                    // Get new balance
                                    let new_balance = ore_client_clone.solana.get_balance(&signer_pubkey).await.unwrap_or(0);

                                    if let Err(e) = discord_clone.notify_claim_sol(accumulated_rewards, new_balance).await {
                                        log::error!("Failed to send Discord claim notification: {}", e);
                                    }
                                }
                                Err(e) => {
                                    log::error!("❌ Failed to claim SOL: {}", e);
                                    if let Err(e) = discord_clone.notify_error(&format!("Failed to claim SOL: {}", e)).await {
                                        log::error!("Failed to send Discord error notification: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("❌ Failed to load keypair for claim: {}", e);
                        }
                    }
                }

                // Send win notification
                // Calculate net profit (earned SOL - all bets in this martingale cycle)
                // This includes the current bet and all previous losing bets in the cycle
                let net_profit = (sol_earned_actual as i64) - (cycle_bet_total as i64);

                log::info!("📊 Martingale cycle summary:");
                log::info!("   Total bet in cycle: {:.6} SOL", cycle_bet_total as f64 / 1e9);
                log::info!("   SOL earned: {:.6} SOL", sol_earned_actual as f64 / 1e9);
                log::info!("   Net profit: {:.6} SOL", net_profit as f64 / 1e9);

                // Update martingale state with actual earnings
                martingale_state_clone.lock().unwrap().update_earnings(ore_earned_actual, sol_earned_actual);

                if let Err(e) = discord_clone.notify_win(
                    round_id,
                    winning_square as u8,
                    ore_earned_actual,
                    sol_earned_actual,
                    net_profit,
                ).await {
                    log::error!("Failed to send Discord win notification: {}", e);
                }

                // Send stats notification if interval reached (after earnings update)
                let stats_interval = config_stats_clone.discord.stats_notification_interval;
                let (total_rounds, win_count, loss_count, win_rate, total_earned_ore, net_profit) = {
                    let state = martingale_state_clone.lock().unwrap();
                    let total_rounds = state.win_count + state.loss_count;
                    (
                        total_rounds,
                        state.win_count,
                        state.loss_count,
                        state.win_rate(),
                        state.total_earned_ore,
                        state.net_profit_sol(),
                    )
                };

                if total_rounds % stats_interval == 0 && total_rounds > 0 {
                    if let Err(e) = discord_stats_clone.notify_stats(
                        total_rounds,
                        win_count,
                        loss_count,
                        win_rate,
                        total_earned_ore,
                        net_profit,
                    ).await {
                        log::error!("Failed to send stats notification: {}", e);
                    }
                }
            });
        } else {
            log::warn!("❌ Lost. Winning square was {}, we bet on {:?}", winning_square, block_indices);

            let (should_continue, should_warn) = {
                let mut state = martingale_state.lock().unwrap();
                state.on_loss(&config.martingale)
            };

            let (consecutive_losses, current_bet_per_block) = {
                let state = martingale_state.lock().unwrap();
                (state.consecutive_losses, state.current_bet_per_block)
            };

            if let Err(e) = discord.notify_loss(
                round_id,
                winning_square as u8,
                consecutive_losses,
                current_bet_per_block,
            ).await {
                log::error!("Failed to send Discord notification: {}", e);
            }

            if should_warn {
                if let Err(e) = discord.notify_warning(
                    consecutive_losses,
                    config.martingale.max_consecutive_losses,
                    current_bet_per_block,
                ).await {
                    log::error!("Failed to send Discord notification: {}", e);
                }
            }

            // Send stats notification if interval reached (after loss)
            let stats_interval = config.discord.stats_notification_interval;
            let (total_rounds, win_count, loss_count, win_rate, total_earned_ore, net_profit) = {
                let state = martingale_state.lock().unwrap();
                let total_rounds = state.win_count + state.loss_count;
                (
                    total_rounds,
                    state.win_count,
                    state.loss_count,
                    state.win_rate(),
                    state.total_earned_ore,
                    state.net_profit_sol(),
                )
            };

            if total_rounds % stats_interval == 0 && total_rounds > 0 {
                if let Err(e) = discord.notify_stats(
                    total_rounds,
                    win_count,
                    loss_count,
                    win_rate,
                    total_earned_ore,
                    net_profit,
                ).await {
                    log::error!("Failed to send stats notification: {}", e);
                }
            }

            if !should_continue {
                return Ok(false);
            }
        }
    } else {
        log::warn!("⚠️ Round RNG not available yet. Will try again next round.");
    }

    Ok(true)
}
