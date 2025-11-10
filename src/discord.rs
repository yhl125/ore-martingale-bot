use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use serde_json::json;

#[derive(Clone)]
pub struct DiscordNotifier {
    webhook_url: String,
    stats_webhook_url: String,
    warn_webhook_url: String,
    client: Client,
}

impl DiscordNotifier {
    pub fn new(webhook_url: String, stats_webhook_url: String, warn_webhook_url: String) -> Self {
        Self {
            webhook_url,
            stats_webhook_url,
            warn_webhook_url,
            client: Client::new(),
        }
    }

    /// Send a bet notification
    pub async fn notify_bet(
        &self,
        round_id: u64,
        blocks: &[u8],
        bet_per_block: u64,
        total_bet: u64,
        consecutive_losses: u8,
    ) -> Result<()> {
        let embed = json!({
            "embeds": [{
                "title": "🎲 New Bet Placed",
                "color": 3447003, // Blue
                "fields": [
                    {
                        "name": "Round",
                        "value": format!("#{}", round_id),
                        "inline": true
                    },
                    {
                        "name": "Blocks",
                        "value": format!("{:?}", blocks),
                        "inline": true
                    },
                    {
                        "name": "Bet per Block",
                        "value": format!("{:.6} SOL", bet_per_block as f64 / 1e9),
                        "inline": true
                    },
                    {
                        "name": "Total Bet",
                        "value": format!("{:.6} SOL", total_bet as f64 / 1e9),
                        "inline": true
                    },
                    {
                        "name": "Consecutive Losses",
                        "value": consecutive_losses.to_string(),
                        "inline": true
                    }
                ],
                "timestamp": Utc::now().to_rfc3339()
            }]
        });

        self.send_webhook(embed).await
    }

    /// Send a win notification
    pub async fn notify_win(
        &self,
        round_id: u64,
        winning_block: u8,
        ore_reward: u64,
        sol_reward: u64,
        net_profit_sol: i64,
    ) -> Result<()> {
        let embed = json!({
            "embeds": [{
                "title": "✅ WIN!",
                "color": 3066993, // Green
                "fields": [
                    {
                        "name": "Round",
                        "value": format!("#{}", round_id),
                        "inline": true
                    },
                    {
                        "name": "Winning Block",
                        "value": winning_block.to_string(),
                        "inline": true
                    },
                    {
                        "name": "ORE Reward",
                        "value": format!("{:.6} ORE", ore_reward as f64 / 1e11),
                        "inline": true
                    },
                    {
                        "name": "SOL Reward",
                        "value": format!("{:.6} SOL", sol_reward as f64 / 1e9),
                        "inline": true
                    },
                    {
                        "name": "Net Profit",
                        "value": format!("{:.6} SOL", net_profit_sol as f64 / 1e9),
                        "inline": true
                    }
                ],
                "timestamp": Utc::now().to_rfc3339()
            }]
        });

        self.send_webhook(embed).await
    }

    /// Send a loss notification
    pub async fn notify_loss(
        &self,
        round_id: u64,
        winning_block: u8,
        consecutive_losses: u8,
        next_bet: u64,
    ) -> Result<()> {
        let embed = json!({
            "embeds": [{
                "title": "❌ Loss",
                "color": 15158332, // Red
                "fields": [
                    {
                        "name": "Round",
                        "value": format!("#{}", round_id),
                        "inline": true
                    },
                    {
                        "name": "Winning Block",
                        "value": winning_block.to_string(),
                        "inline": true
                    },
                    {
                        "name": "Consecutive Losses",
                        "value": consecutive_losses.to_string(),
                        "inline": true
                    },
                    {
                        "name": "Next Bet",
                        "value": format!("{:.6} SOL per block", next_bet as f64 / 1e9),
                        "inline": true
                    }
                ],
                "timestamp": Utc::now().to_rfc3339()
            }]
        });

        self.send_webhook(embed).await
    }

    /// Send a warning notification (to stats channel)
    pub async fn notify_warning(
        &self,
        consecutive_losses: u8,
        max_losses: u8,
        current_bet: u64,
    ) -> Result<()> {
        let embed = json!({
            "embeds": [{
                "title": "⚠️ Warning: High Consecutive Losses",
                "color": 15105570, // Orange
                "fields": [
                    {
                        "name": "Consecutive Losses",
                        "value": format!("{}/{}", consecutive_losses, max_losses),
                        "inline": true
                    },
                    {
                        "name": "Current Bet",
                        "value": format!("{:.6} SOL per block", current_bet as f64 / 1e9),
                        "inline": true
                    },
                    {
                        "name": "Status",
                        "value": format!("Approaching max loss limit!"),
                        "inline": false
                    }
                ],
                "timestamp": Utc::now().to_rfc3339()
            }]
        });

        self.send_webhook_to_warn(embed).await
    }

    /// Send an error notification
    pub async fn notify_error(&self, error_msg: &str) -> Result<()> {
        let embed = json!({
            "embeds": [{
                "title": "🚨 Error",
                "color": 10038562, // Dark Red
                "description": error_msg,
                "timestamp": Utc::now().to_rfc3339()
            }]
        });

        self.send_webhook(embed).await
    }

    /// Send SOL claim notification
    pub async fn notify_claim_sol(
        &self,
        claimed_amount: u64,
        new_balance: u64,
    ) -> Result<()> {
        let embed = json!({
            "embeds": [{
                "title": "💰 SOL Claimed",
                "color": 15844367, // Gold
                "fields": [
                    {
                        "name": "Claimed Amount",
                        "value": format!("{:.6} SOL", claimed_amount as f64 / 1e9),
                        "inline": true
                    },
                    {
                        "name": "New Balance",
                        "value": format!("{:.6} SOL", new_balance as f64 / 1e9),
                        "inline": true
                    }
                ],
                "timestamp": Utc::now().to_rfc3339()
            }]
        });

        self.send_webhook(embed).await
    }

    /// Send statistics summary
    pub async fn notify_stats(
        &self,
        total_rounds: u32,
        win_count: u32,
        loss_count: u32,
        win_rate: f64,
        total_earned_ore: u64,
        net_profit_sol: i64,
    ) -> Result<()> {
        let embed = json!({
            "embeds": [{
                "title": "📊 Bot Statistics",
                "color": 9807270, // Purple
                "fields": [
                    {
                        "name": "Total Rounds",
                        "value": total_rounds.to_string(),
                        "inline": true
                    },
                    {
                        "name": "Wins",
                        "value": win_count.to_string(),
                        "inline": true
                    },
                    {
                        "name": "Losses",
                        "value": loss_count.to_string(),
                        "inline": true
                    },
                    {
                        "name": "Win Rate",
                        "value": format!("{:.2}%", win_rate),
                        "inline": true
                    },
                    {
                        "name": "Total ORE Earned",
                        "value": format!("{:.6} ORE", total_earned_ore as f64 / 1e11),
                        "inline": true
                    },
                    {
                        "name": "Net Profit",
                        "value": format!("{:.6} SOL", net_profit_sol as f64 / 1e9),
                        "inline": true
                    }
                ],
                "timestamp": Utc::now().to_rfc3339()
            }]
        });

        self.send_webhook_to_stats(embed).await
    }

    async fn send_webhook(&self, payload: serde_json::Value) -> Result<()> {
        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Discord webhook failed: {} - {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(())
    }

    async fn send_webhook_to_stats(&self, payload: serde_json::Value) -> Result<()> {
        let response = self
            .client
            .post(&self.stats_webhook_url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Discord stats webhook failed: {} - {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(())
    }

    async fn send_webhook_to_warn(&self, payload: serde_json::Value) -> Result<()> {
        let response = self
            .client
            .post(&self.warn_webhook_url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Discord warn webhook failed: {} - {}",
                response.status(),
                response.text().await?
            );
        }

        Ok(())
    }
}
