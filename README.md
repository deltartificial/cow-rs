# cow-rs

[![lint](https://img.shields.io/github/actions/workflow/status/deltartificial/cow-rs/lint.yml?branch=main&label=lint&logo=github)](https://github.com/deltartificial/cow-rs/actions/workflows/lint.yml)
[![test](https://img.shields.io/github/actions/workflow/status/deltartificial/cow-rs/test.yml?branch=main&label=test&logo=github)](https://github.com/deltartificial/cow-rs/actions/workflows/test.yml)
[![bench](https://img.shields.io/github/actions/workflow/status/deltartificial/cow-rs/bench.yml?branch=main&label=bench&logo=github)](https://github.com/deltartificial/cow-rs/actions/workflows/bench.yml)
[![codecov](https://codecov.io/gh/deltartificial/cow-rs/graph/badge.svg)](https://codecov.io/gh/deltartificial/cow-rs)
[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](#license)

Rust SDK for the [CoW Protocol](https://cow.fi).

A complete, type-safe Rust port of the CoW Protocol TypeScript SDK — covering order placement, signing, quoting, composable orders (TWAP, stop-loss), on-chain reading, subgraph queries, and more.

## Modules

| Module          | Purpose                                                          |
| --------------- | ---------------------------------------------------------------- |
| `config`        | Chain IDs, contract addresses, token constants                   |
| `order_book`    | Orderbook HTTP client and API types                              |
| `order_signing` | EIP-712 digest and ECDSA signing                                 |
| `trading`       | High-level `TradingSdk` and fee-breakdown types                  |
| `app_data`      | Order metadata schema and keccak256 hashing                      |
| `subgraph`      | Historical trading data via GraphQL                              |
| `composable`    | Conditional orders (TWAP, GAT, stop-loss) and Merkle multiplexer |
| `onchain`       | On-chain reading via JSON-RPC `eth_call`                         |
| `permit`        | EIP-2612 permit signing and hook building                        |
| `ethflow`       | Native ETH order flow                                            |
| `bridging`      | Cross-chain bridging                                             |
| `erc20`         | ERC-20 calldata encoding                                         |
| `weiroll`       | Weiroll scripting for batch operations                           |
| `cow_shed`      | CowShed hook framework                                           |
| `flash_loans`   | Flash loan integration                                           |

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
cow-rs = "0.1"
```

### Place a swap order

```rust,no_run
use alloy_primitives::U256;
use cow_rs::{OrderKind, SupportedChainId, TradeParameters, TradingSdk, TradingSdkConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdk = TradingSdk::new(
        TradingSdkConfig::prod(SupportedChainId::Sepolia, "MyApp"),
        "0xYOUR_PRIVATE_KEY",
    )?;

    let result = sdk
        .post_swap_order(TradeParameters {
            kind: OrderKind::Sell,
            sell_token: "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14".parse()?,
            sell_token_decimals: 18,
            buy_token: "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238".parse()?,
            buy_token_decimals: 6,
            amount: U256::from(100_000_000_000_000_u64),
            slippage_bps: Some(50),
            receiver: None,
            valid_for: None,
            valid_to: None,
            partially_fillable: None,
            partner_fee: None,
        })
        .await?;

    println!("Order placed: {}", result.order_id);
    Ok(())
}
```

## Development

```bash
cargo build --workspace      # build
cargo nextest run --workspace # tests
cargo test --doc --workspace  # doctests
make lint                     # all linters
make pr                       # full pre-PR check
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
