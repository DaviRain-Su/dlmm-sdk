# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Solana-based market making bot for Dynamic Liquidity Market Maker (DLMM) protocol. The bot manages liquidity positions, monitors price ranges, and automatically rebalances positions based on market conditions.

## Key Commands

### Build
```bash
cargo build
```

### Run Tests
```bash
cargo test
```

### Linting
```bash
cargo clippy
```

### Type Checking
```bash
cargo check
```

### Run the Bot
```bash
# Development/local testing
target/debug/market_making --provider <CLUSTER> --wallet <WALLET_PATH> --config_file <CONFIG_PATH>

# View-only mode (no wallet required)
target/debug/market_making --provider <CLUSTER> --user_public_key <PUBKEY> --config_file <CONFIG_PATH>
```

## Architecture

### Core Components

1. **Core Module** (`src/core.rs`)
   - Central orchestrator managing RPC client, wallet, and state
   - Handles position refresh, price range monitoring, and liquidity management
   - Implements automated rebalancing logic based on market conditions

2. **State Management** (`src/state.rs`)
   - `AllPosition`: Manages all positions across configured trading pairs
   - `SinglePosition`: Tracks individual pair positions with bin arrays and price ranges
   - Implements slippage calculations and position value computations

3. **Configuration** (`src/pair_config.rs`)
   - Loads trading pair configurations from JSON
   - Defines market making modes: ModeRight, ModeLeft, ModeBoth, ModeView
   - ModeView allows monitoring without executing trades

4. **HTTP Router** (`src/router.rs`)
   - Exposes REST endpoints for position monitoring
   - `/check_positions` endpoint returns current position states

5. **Bin Array Manager** (`src/bin_array_manager.rs`)
   - Manages DLMM bin arrays for price range positioning
   - Handles bin liquidity calculations and position updates

## Configuration Structure

The bot uses a JSON configuration file (`src/config.json`) with the following structure:
- `pair_address`: DLMM pair public key
- `x_amount`: Amount of token X for liquidity provision
- `y_amount`: Amount of token Y for liquidity provision
- `mode`: Operating mode (ModeBoth, ModeLeft, ModeRight, ModeView)

## Key Operations

The bot runs two main background tasks:
1. **State Refresh** (60-second interval): Updates position states and bin arrays
2. **Price Range Monitoring** (60-second interval): Checks if positions need rebalancing when price moves outside range

## Development Notes

- Uses Anchor framework for Solana program interactions
- Requires Rust toolchain 1.76.0 (1.76.0-x86_64-apple-darwin for M1 Macs)
- All amounts are in base units (lamports for SOL, smallest decimal for tokens)
- Position monitoring available at `http://localhost:8080/check_positions`

## Git Workflow

After completing each task:
1. Create a git commit with a descriptive message summarizing the changes
2. The commit message should focus on what was accomplished and why
3. Include a summary of the work done in the commit message
