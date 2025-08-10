# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a DLMM (Dynamic Liquidity Market Maker) CLI tool built in Rust for interacting with the DLMM Solana program. The CLI provides commands for managing liquidity pairs, positions, and performing swaps on the DLMM protocol.

## Build and Development Commands

### Build
```bash
cargo build
```

### Run (after building)
```bash
target/debug/cli --help
```

### Check code (fast compilation check)
```bash
cargo check
```

### Lint with Clippy
```bash
cargo clippy
```

### Run tests
```bash
cargo test
```

### Run a specific test
```bash
cargo test test_name
```

## Architecture

### Core Structure

The CLI follows a command-pattern architecture with the following key components:

1. **Main Entry Point** (`src/main.rs`):
   - Parses CLI arguments using clap
   - Sets up Anchor client with Solana RPC connection
   - Routes commands to appropriate instruction handlers
   - Handles transaction configuration and compute unit price settings

2. **Command Arguments** (`src/args.rs`):
   - Defines all CLI commands using clap derive macros
   - Separates regular user commands (DLMMCommand) from admin commands (AdminCommand)
   - Provides global configuration overrides (cluster, wallet, priority fee)

3. **Instructions Module** (`src/instructions/`):
   - Each instruction has its own file implementing the execution logic
   - Common patterns: parameter validation, account derivation, transaction building and sending
   - Organized into submodules:
     - `admin/`: Admin-only operations
     - `ilm/`: Initial Liquidity Management operations
     - Root level: Core user operations

4. **Math Utilities** (`src/math.rs`):
   - Price and bin calculations specific to DLMM protocol

### Key Patterns

1. **Instruction Execution Pattern**:
   - Each instruction follows: `execute_<instruction_name>(params, program, transaction_config)`
   - Uses Anchor client for account resolution and transaction building
   - Handles compute unit price instructions when provided

2. **Account Derivation**:
   - PDAs (Program Derived Addresses) are derived using seeds from the commons crate
   - Common PDAs: lb_pair, position, bin_array, oracle

3. **Error Handling**:
   - Uses anyhow::Result for error propagation
   - Retry logic implemented for certain operations (e.g., SeedLiquidityByOperator)

4. **Dependencies**:
   - Workspace structure with shared `commons` crate for common utilities
   - Uses Anchor framework for Solana program interaction
   - dlmm program types imported from workspace

## Toolchain Requirements

- Rust 1.76.0
- For M1 Macs: Use x86_64-apple-darwin target triple (e.g., 1.76.0-x86_64-apple-darwin)

## Common Development Tasks

When adding new instructions:
1. Add command variant to `DLMMCommand` or `AdminCommand` enum in `src/args.rs`
2. Create new file in `src/instructions/` with execute function
3. Add module export in `src/instructions/mod.rs`
4. Add match arm in `src/main.rs` to route the command

## Git Workflow

After completing each task:
1. Create a git commit with a descriptive message summarizing the changes
2. The commit message should focus on what was accomplished and why
3. Include a summary of the work done in the commit message