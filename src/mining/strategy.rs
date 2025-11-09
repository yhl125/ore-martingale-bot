use crate::config::MartingaleConfig;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MartingaleState {
    pub current_round: u64,
    pub current_bet_per_block: u64,  // Current SOL bet amount
    pub consecutive_losses: u8,
    pub total_bet_lamports: u64,    // Total SOL bet (lost)
    pub current_cycle_bet_lamports: u64, // Total bet in current martingale cycle (resets on win)
    pub total_earned_ore: u64,         // Total ORE earned (in smallest unit)
    pub total_earned_sol: u64,         // Total SOL recovered from winning (actual rewards after fees)
    pub last_win_time: Option<i64>,
    pub win_count: u32,
    pub loss_count: u32,
}

impl MartingaleState {
    pub fn new(base_bet: u64) -> Self {
        Self {
            current_round: 0,
            current_bet_per_block: base_bet,
            consecutive_losses: 0,
            total_bet_lamports: 0,
            current_cycle_bet_lamports: 0,
            total_earned_ore: 0,
            total_earned_sol: 0,
            last_win_time: None,
            win_count: 0,
            loss_count: 0,
        }
    }

    /// Called when winning a round
    pub fn on_win(
        &mut self,
        config: &MartingaleConfig,
        ore_reward: u64,
        sol_reward: u64,
    ) {
        log::info!("✅ WON Round! ORE: {}, SOL Recovered: {}",
            ore_reward, sol_reward);

        self.total_earned_ore += ore_reward;
        self.total_earned_sol += sol_reward;
        self.consecutive_losses = 0;
        self.current_cycle_bet_lamports = 0; // Reset cycle bet on win
        self.last_win_time = Some(chrono::Utc::now().timestamp());
        self.win_count += 1;

        // Reset to base bet after win
        self.current_bet_per_block = config.base_bet_lamports();
    }

    /// Called when losing a round
    /// Returns (should_continue, should_warn)
    pub fn on_loss(&mut self, config: &MartingaleConfig) -> (bool, bool) {
        log::warn!("❌ LOST Round #{}", self.consecutive_losses + 1);

        self.consecutive_losses += 1;
        self.loss_count += 1;

        // Check if warning threshold reached
        let should_warn = self.consecutive_losses == config.warn_consecutive_losses;

        // Check if max consecutive losses reached
        if self.consecutive_losses >= config.max_consecutive_losses {
            log::error!("🛑 Max consecutive losses reached. Resetting bet.");
            self.reset(config);
            return (false, should_warn); // Don't continue, signal warning
        }

        // Apply martingale: always double the bet (2x multiplier)
        let new_bet = self.current_bet_per_block * 2;
        self.current_bet_per_block = new_bet;

        log::info!(
            "📈 Martingale: Doubling bet {:.6} → {:.6} SOL",
            (self.current_bet_per_block / 2) as f64 / 1e9,
            new_bet as f64 / 1e9
        );

        (true, should_warn) // Continue betting, signal warning if needed
    }

    /// Record bet placement
    pub fn record_bet(&mut self, total_bet: u64) {
        self.total_bet_lamports += total_bet;
        self.current_cycle_bet_lamports += total_bet;
    }

    pub fn reset(&mut self, config: &MartingaleConfig) {
        self.consecutive_losses = 0;
        self.current_bet_per_block = config.base_bet_lamports();
        self.current_cycle_bet_lamports = 0; // Reset cycle bet on reset
    }

    pub fn net_profit_sol(&self) -> i64 {
        (self.total_earned_sol as i64) - (self.total_bet_lamports as i64)
    }

    pub fn win_rate(&self) -> f64 {
        let total_rounds = self.win_count + self.loss_count;
        if total_rounds == 0 {
            return 0.0;
        }
        (self.win_count as f64 / total_rounds as f64) * 100.0
    }
}
