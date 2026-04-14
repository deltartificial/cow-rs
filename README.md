# cow-rs

[![crates.io](https://img.shields.io/crates/v/cow-rs.svg)](https://crates.io/crates/cow-rs)
[![docs.rs](https://img.shields.io/docsrs/cow-rs)](https://docs.rs/cow-rs)
[![lint](https://img.shields.io/github/actions/workflow/status/deltartificial/cow-rs/lint.yml?branch=main&label=lint&logo=github)](https://github.com/deltartificial/cow-rs/actions/workflows/lint.yml)
[![test](https://img.shields.io/github/actions/workflow/status/deltartificial/cow-rs/test.yml?branch=main&label=test&logo=github)](https://github.com/deltartificial/cow-rs/actions/workflows/test.yml)
[![bench](https://img.shields.io/github/actions/workflow/status/deltartificial/cow-rs/bench.yml?branch=main&label=bench&logo=github)](https://github.com/deltartificial/cow-rs/actions/workflows/bench.yml)
[![codecov](https://codecov.io/gh/deltartificial/cow-rs/graph/badge.svg)](https://codecov.io/gh/deltartificial/cow-rs)
[![msrv](https://img.shields.io/badge/msrv-1.93-blue?logo=rust)](#minimum-supported-rust-version)
[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](#license)

A standalone Rust SDK for the [CoW Protocol](https://cow.fi), the intent-based
DEX aggregator that settles trades through batch auctions with MEV protection.

`cow-rs` is a complete, type-safe port of the official
[TypeScript SDK](https://github.com/cowprotocol/cow-sdk). It covers the full
trading lifecycle (quoting, signing, placing, tracking, and cancelling orders)
and exposes lower-level building blocks so you can compose your own flows:
conditional orders (TWAP, stop-loss), on-chain reads via `eth_call`, subgraph
queries, EIP-2612 permits, ethflow, bridging, CowShed hooks, flash loans and
Weiroll scripts.

It runs natively and compiles to WebAssembly for use in the browser.

## Table of Contents

- [Overview](#overview)
- [Supported Chains](#supported-chains)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Modules](#modules)
- [Examples](#examples)
- [Feature Flags](#feature-flags)
- [WebAssembly](#webassembly)
- [Architecture](#architecture)
- [Development](#development)
- [Minimum Supported Rust Version](#minimum-supported-rust-version)
- [Contributing](#contributing)
- [License](#license)

## Overview

CoW Protocol matches trades peer-to-peer when possible (Coincidence of Wants)
and falls back to on-chain liquidity for the rest. Orders are signed off-chain,
submitted to an orderbook, and settled in batches by solvers competing for the
best execution price. This design eliminates MEV extraction on signed orders,
removes slippage from the user's perspective, and saves gas on matched volume.

`cow-rs` gives you a single crate to interact with every piece of that stack
from Rust:

- A high-level `TradingSdk` for the common path (quote, sign, post, wait).
- Typed HTTP clients for the orderbook and subgraph APIs.
- EIP-712 signing with no opaque dependencies on JavaScript toolchains.
- Composable primitives for advanced order types and custom settlement hooks.

## Supported Chains

| Chain         | Chain ID   | Network     |
| ------------- | ---------- | ----------- |
| Ethereum      | `1`        | Mainnet     |
| Gnosis Chain  | `100`      | Mainnet     |
| Arbitrum One  | `42161`    | Mainnet     |
| Base          | `8453`     | Mainnet     |
| Polygon       | `137`      | Mainnet     |
| Avalanche     | `43114`    | Mainnet     |
| BNB Chain     | `56`       | Mainnet     |
| Sepolia       | `11155111` | Testnet     |

See [`SupportedChainId`](https://docs.rs/cow-rs/latest/cow_rs/config/enum.SupportedChainId.html)
for the authoritative list.

## Installation

```toml
[dependencies]
cow-rs = "1.0"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
alloy-primitives = "1"
```

## Quick Start

### Post a swap order

Fetches a quote, signs an EIP-712 order with your key, and submits it to the
CoW orderbook.

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
            sell_token: "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14".parse()?, // WETH
            sell_token_decimals: 18,
            buy_token: "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238".parse()?,  // USDC
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

    println!("order placed: {}", result.order_id);
    Ok(())
}
```

### Fetch a quote without signing

Useful when you want to display an indicative price before the user commits.

```rust,no_run
use alloy_primitives::U256;
use cow_rs::{OrderKind, SupportedChainId, TradeParameters, TradingSdk, TradingSdkConfig};

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let sdk = TradingSdk::new(
    TradingSdkConfig::prod(SupportedChainId::Mainnet, "MyApp"),
    "0xYOUR_PRIVATE_KEY",
)?;

let quote = sdk.get_quote(TradeParameters {
    kind: OrderKind::Sell,
    sell_token: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?,
    sell_token_decimals: 18,
    buy_token: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse()?,
    buy_token_decimals: 6,
    amount: U256::from(10u128.pow(18)),
    slippage_bps: Some(50),
    receiver: None, valid_for: None, valid_to: None,
    partially_fillable: None, partner_fee: None,
}).await?;

println!("expected out: {}", quote.amounts_and_costs.after_slippage.buy_amount);
# Ok(())
# }
```

## Modules

| Module            | Purpose                                                            |
| ----------------- | ------------------------------------------------------------------ |
| `config`          | Chain IDs, contract addresses, token constants                     |
| `order_book`      | Orderbook HTTP client and API types                                |
| `order_signing`   | EIP-712 digest and ECDSA signing                                   |
| `trading`         | High-level `TradingSdk` and fee-breakdown types                    |
| `app_data`        | Order metadata schema and keccak256 hashing                        |
| `subgraph`        | Historical trading data via GraphQL                                |
| `composable`      | Conditional orders (TWAP, GAT, stop-loss) and Merkle multiplexer   |
| `onchain`         | On-chain reading via JSON-RPC `eth_call`                           |
| `permit`          | EIP-2612 permit signing and hook building                          |
| `ethflow`         | Native ETH order flow                                              |
| `bridging`        | Cross-chain bridging                                               |
| `erc20`           | ERC-20 calldata encoding                                           |
| `weiroll`         | Weiroll scripting for batch operations                             |
| `cow_shed`        | CowShed hook framework                                             |
| `flash_loans`     | Flash loan integration                                             |
| `browser_wallet`  | Wallet bridge for WASM targets                                     |

Generated API docs live on [docs.rs](https://docs.rs/cow-rs).

## Examples

Runnable examples live under [`examples/native/examples`](./examples/native/examples),
grouped by theme:

```bash
cargo run -p examples-native --example swap_order
cargo run -p examples-native --example get_quote
cargo run -p examples-native --example twap
cargo run -p examples-native --example stop_loss
cargo run -p examples-native --example order_status
```

| Folder          | What it shows                                               |
| --------------- | ----------------------------------------------------------- |
| `orders/`       | quote, swap, limit, signing, status, cancellation           |
| `composable/`   | TWAP and stop-loss conditional orders                       |
| `config/`       | picking a chain and reading its constants                   |
| `onchain/`      | reading balances and allowances via `eth_call`              |
| `subgraph/`     | historical data queries                                     |
| `permit/`       | EIP-2612 approve-by-signature                               |
| `cow_shed/`     | pre- and post-trade hooks                                   |
| `flash_loans/`  | borrowing inside a settlement                               |
| `bridging/`     | cross-chain intents                                         |
| `weiroll/`      | batched call scripting                                      |
| `erc20/`        | calldata encoding helpers                                   |
| `app_data/`     | building and hashing order metadata                         |
| `ethflow/`      | trading native ETH                                          |

A separate WebAssembly example lives in [`examples/wasm`](./examples/wasm).

## Feature Flags

| Flag       | Default | Description                                                |
| ---------- | :-----: | ---------------------------------------------------------- |
| `native`   |   on    | Native (tokio + reqwest) HTTP transport                    |
| `wasm`     |   off   | `wasm-bindgen` glue, browser wallet bridge, fetch client   |

Enable WASM builds with:

```toml
cow-rs = { version = "1.0", default-features = false, features = ["wasm"] }
```

## WebAssembly

`cow-rs` compiles to `wasm32-unknown-unknown` and exposes a browser-wallet
bridge so the signing step can be delegated to an injected provider
(MetaMask, Rabby, etc.) instead of a raw private key. See
[`examples/wasm`](./examples/wasm) for an end-to-end setup.

## Architecture

Single-crate workspace. All public API lives in `crates/cow-rs`; the repo
ships additional members for examples and fuzz targets:

```
cow-rs/
├── crates/
│   └── cow-rs/        # the SDK itself
├── examples/
│   ├── native/        # cargo examples (tokio)
│   └── wasm/          # browser example
├── fuzz/              # cargo-fuzz targets
├── specs/             # conformance fixtures against the TypeScript SDK
└── docs/adr/          # architecture decision records
```

Design notes and trade-offs are recorded as ADRs in [`docs/adr`](./docs/adr).

## Development

The toolchain is strict and opinionated: nightly clippy with nursery lints,
nightly rustfmt, `cargo-deny`, `typos`, `dprint` and `nextest`.

```bash
cargo build --workspace          # build everything
cargo nextest run --workspace    # unit + integration tests
cargo test --doc --workspace     # doctests (nextest does not run these)
make lint                        # fmt, clippy, typos, deny, dprint
make pr                          # full pre-PR gate
make maxperf                     # fat-LTO release build
```

Conformance tests replay fixtures produced by the TypeScript SDK to guarantee
byte-for-byte parity on signing and encoding. See [`specs/`](./specs).

## Minimum Supported Rust Version

The MSRV is **1.93**. It is enforced in CI and will only be bumped in a
minor release, never in a patch.

## Contributing

Contributions are welcome. Before opening a PR:

1. Run `make pr` locally. It mirrors the CI gate.
2. Make sure `cargo doc --workspace --all-features --no-deps --document-private-items`
   builds without warnings.
3. Use [Conventional Commits](https://www.conventionalcommits.org/) for both
   commit messages and PR titles (`feat`, `fix`, `docs`, `refactor`, …).

The project config (clippy lints, fmt rules, deny policy) is deliberately
strict. If you hit a rule that feels wrong for your change, open an issue
first; don't relax the config in your PR.

Bug reports and feature requests go to
[GitHub issues](https://github.com/deltartificial/cow-rs/issues). For protocol
questions, the [CoW Protocol docs](https://docs.cow.fi) and
[Discord](https://discord.gg/cowprotocol) are the right places to look.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT), at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in this
crate, as defined in the Apache-2.0 license, shall be dual-licensed as above,
without any additional terms or conditions.
