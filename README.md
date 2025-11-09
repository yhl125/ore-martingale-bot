# Ore Martingale Bot

Automated betting bot for Solana's [ORE](https://ore.supply/) using martingale strategy.

## Overview

Ore Martingale Bot is a fully automated trading system that:

- 🎯 Places strategic bets on ORE's 5x5 grid
- 📈 Implements martingale betting strategy with configurable risk management
- 🔔 Sends real-time Discord notifications for all betting events
- 💰 Auto-claims accumulated rewards when threshold is reached
- ⚡ Uses WebSocket subscriptions for instant round updates
- 🛡️ Includes safety features like balance monitoring and loss limits

## How It Works

### ORE Mechanics

- **5x5 Grid**: 25 blocks per round (~1 minute each)
- **SOL Betting**: Stake SOL on one or multiple blocks
- **Random Winner**: One winning block selected randomly per round
- **Prize Pool**: Losers' SOL redistributed proportionally to winners
- **ORE Rewards**: Winners earn ORE tokens as bonus

### Martingale Strategy

The bot uses classic martingale betting logic:

1. **Base Bet**: Starts with small bet (e.g., 0.001 SOL per block)
2. **Double on Loss**: Doubles bet amount after each loss
3. **Reset on Win**: Returns to base bet after winning
4. **Loss Limit**: Stops automatically after max consecutive losses

**Example Progression:**
```
Round 1: 0.001 SOL → Lost  → Next: 0.002 SOL
Round 2: 0.002 SOL → Lost  → Next: 0.004 SOL
Round 3: 0.004 SOL → Won   → Next: 0.001 SOL (reset)
```

### Built-in Risk Management

- ✅ Maximum consecutive loss limit
- ✅ Warning threshold notifications
- ✅ Minimum balance monitoring
- ✅ Auto-pause when limits reached
- ✅ Real-time profit/loss tracking

## Quick Start

### Prerequisites

- **Rust 1.70+** - Install via [rustup](https://rustup.rs/)
- **Solana Wallet** - With SOL balance for betting and transaction fees
- **Discord Webhook** - For real-time notifications ([Setup Guide](https://support.discord.com/hc/en-us/articles/228383668-Intro-to-Webhooks))

### Installation

1. **Clone Repository**
   ```bash
   git clone https://github.com/yhl125/ore-martingale-bot.git
   cd ore-martingale-bot
   ```

2. **Configure Bot**
   ```bash
   cp config.example.json config.json
   vim config.json  # Edit with your settings
   ```

3. **Build & Run**
   ```bash
   # Build optimized binary
   cargo build --release

   # Run with logging
   RUST_LOG=info cargo run --release
   ```

### First Run Checklist

- [ ] SOL balance > minimum threshold (default: 0.1 SOL)
- [ ] Valid private key configured
- [ ] Discord webhook URLs set up
- [ ] Risk parameters configured (base bet, max losses)

## Configuration

### Configuration File Structure

```json
{
  "rpc_url": "https://api.mainnet-beta.solana.com",
  "private_key": "YOUR_BASE58_PRIVATE_KEY_HERE",
  "martingale": { ... },
  "monitoring": { ... },
  "discord": { ... }
}
```

### Core Settings

| Parameter | Type | Description |
|-----------|------|-------------|
| `rpc_url` | string | Solana RPC endpoint |
| `private_key` | string | Base58 encoded private key |

### Martingale Parameters

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `base_bet_amount` | float | 0.001-1.0 | Starting bet per block (SOL) |
| `max_consecutive_losses` | int | 5-15 | Stop after N consecutive losses |
| `warn_consecutive_losses` | int | 3-12 | Warning threshold before max |
| `blocks_per_bet` | int | 1-25 | Number of blocks per round |

**Block Selection Strategy:**

- Randomly select N blocks each round

### Monitoring Settings

| Parameter | Type | Description |
|-----------|------|-------------|
| `min_balance_sol` | float | Minimum SOL balance before pause |
| `auto_claim_sol_threshold` | float | Auto-claim rewards at this amount |

### Discord Webhooks

The bot supports three separate webhook endpoints for different notification types:

```json
{
  "discord": {
    "webhook_url": "https://discord.com/api/webhooks/...",
    "stats_webhook_url": "https://discord.com/api/webhooks/...",
    "warn_webhook_url": "https://discord.com/api/webhooks/..."
  }
}
```

**Webhook Channels:**

- **webhook_url** - General notifications (bet placed, win, loss, error, claim)
- **stats_webhook_url** - Statistics summaries (periodic reports every 10 rounds)
- **warn_webhook_url** - Warning alerts (consecutive loss warnings)

**Notification Types:**

- 🎲 **Bet Placed** → `webhook_url` - Round ID, blocks selected, bet amount, consecutive losses
- ✅ **Win** → `webhook_url` - Winning block, ORE earned, SOL earned, net profit
- ❌ **Loss** → `webhook_url` - Winning block, consecutive losses, next bet amount
- ⚠️ **Warning** → `warn_webhook_url` - Loss streak approaching limit
- 🚨 **Error** → `webhook_url` - Critical issues (low balance, max losses)
- 📊 **Stats** → `stats_webhook_url` - Periodic summary (every 10 rounds)
- 💰 **Claim** → `webhook_url` - SOL auto-claim executed

## Features

### Core Functionality

✅ **Automated Betting Loop**
- Continuous round monitoring with WebSocket subscriptions
- Automatic bet placement with retry logic (max 3 attempts)
- Dynamic wait time calculation based on round timing

✅ **Smart Transaction Management**
- Auto-checkpoint detection and batching
- Combined Checkpoint+Deploy transactions (gas optimization)
- Signature tracking and confirmation

✅**Real-time Reward Tracking**
- WebSocket-first reward updates (fast)
- RPC fallback with retry mechanism (10 attempts, 2s interval)
- Automatic SOL claim when threshold reached

✅ **Risk Management**
- Balance monitoring before each round
- Automatic pause on low balance or max losses
- Configurable warning thresholds

✅ **Statistics Tracking**
- Win/loss counting and win rate calculation
- Net profit tracking (SOL and ORE)
- Total bet amount tracking per martingale cycle
- Periodic stats reporting (every 10 rounds)

### Technical Features

🔧 **Performance Optimizations**
- Asynchronous reward processing (non-blocking)
- Parallel transaction building and signing
- WebSocket subscriptions for instant updates
- RPC call batching where possible

🔧 **Error Handling**
- Comprehensive retry logic for RPC calls
- Graceful degradation (WebSocket → RPC fallback)
- Transaction failure recovery
- Detailed error logging and Discord alerts

🔧 **Logging System**
- `env_logger` integration with configurable levels
- Structured logging for all major events
- Transaction signatures logged for verification
- Performance timing logs

## Running the Bot

### Command Line Options

```bash
# Standard run with info logging
RUST_LOG=info cargo run --release

# Debug mode (verbose logging)
RUST_LOG=debug cargo run --release

# Run in background (Linux/macOS)
nohup cargo run --release > bot.log 2>&1 &

# Stop background process
pkill -f ore-martingale-bot
```

### Monitoring Bot Activity

1. **Discord Notifications** - Real-time updates in your Discord channel
2. **Console Logs** - Live output showing round-by-round activity
3. **Solana Explorer** - Verify transactions using signature URLs

Example log output:
```
[INFO] 🚀 Ore Martingale Bot starting...
[INFO] ✅ Connected to Solana RPC
[INFO] ✅ Loaded keypair: 7xKX...
[INFO] 💰 Balance: 1.234567 SOL
[INFO] 🎲 Betting on blocks: [3, 12, 18]
[INFO] 💰 Bet: 0.001000 SOL per block, total: 0.003000 SOL
[INFO] ✅ Bet placed successfully!
[INFO] ⏳ Waiting for round #1234 to complete...
[INFO] ✅ WE WON!
[INFO] 💰 SOL earned: 0.012000 SOL
```

## Project Structure

```
ore-martingale-bot/
├── src/
│   ├── main.rs              # Entry point, main betting loop
│   ├── config.rs            # Configuration loading & validation
│   ├── client.rs            # Solana RPC client wrapper
│   ├── keypair.rs           # Private key loading (Base58)
│   ├── discord.rs           # Discord webhook client
│   ├── subscription.rs      # WebSocket miner account subscription
│   ├── mining/
│   │   ├── mod.rs
│   │   ├── strategy.rs      # Martingale state machine
│   │   ├── grid.rs          # Block selection
│   │   └── executor.rs      # Transaction builder & executor
│   └── ore/
│       ├── mod.rs
│       ├── pda.rs           # Program Derived Addresses
│       ├── state.rs         # Board/Round/Miner state structs
│       └── instruction.rs   # ORE instructions
├── config.example.json      # Example configuration
├── Cargo.toml               # Dependencies
└── README.md
```

## Risk Warning

⚠️ **Use at your own risk. You may lose your funds.**

This software is provided "as is" without warranty. The developers assume no liability for any financial losses.

## Troubleshooting

### Common Issues

**Bot stops with "Balance too low"**
- Top up your wallet with SOL
- Adjust `min_balance_sol` in config

**"Max consecutive losses reached"**
- Review martingale settings (lower `base_bet_amount` or increase `max_consecutive_losses`)
- Consider taking a break to avoid emotional decisions

**Discord notifications not working**
- Verify webhook URLs are correct
- Check Discord server permissions
- Test webhook with curl: `curl -X POST -H "Content-Type: application/json" -d '{"content":"test"}' YOUR_WEBHOOK_URL`

**Transactions failing**
- Check RPC endpoint status and rate limits
- Verify sufficient SOL for transaction fees (~0.000005 SOL per tx)
- Consider using paid RPC for better reliability

### Getting Help

- Check logs with `RUST_LOG=debug`
- Verify transaction signatures on Solana Explorer
- Review Discord error notifications for details
