# cow-rs

[![crates.io](https://img.shields.io/crates/v/cow-rs.svg)](https://crates.io/crates/cow-rs)
[![docs.rs](https://img.shields.io/docsrs/cow-rs)](https://docs.rs/cow-rs)
[![lint](https://img.shields.io/github/actions/workflow/status/deltartificial/cow-rs/lint.yml?branch=main&label=lint&logo=github)](https://github.com/deltartificial/cow-rs/actions/workflows/lint.yml)
[![test](https://img.shields.io/github/actions/workflow/status/deltartificial/cow-rs/test.yml?branch=main&label=test&logo=github)](https://github.com/deltartificial/cow-rs/actions/workflows/test.yml)
[![bench](https://img.shields.io/github/actions/workflow/status/deltartificial/cow-rs/bench.yml?branch=main&label=bench&logo=github)](https://github.com/deltartificial/cow-rs/actions/workflows/bench.yml)
[![codecov](https://codecov.io/gh/deltartificial/cow-rs/graph/badge.svg)](https://codecov.io/gh/deltartificial/cow-rs)

A standalone Rust SDK for the [CoW Protocol](https://cow.fi), the intent-based
DEX aggregator that settles trades through batch auctions with MEV protection.

`cow-rs` is a complete, type-safe port of the official
[TypeScript SDK](https://github.com/cowprotocol/cow-sdk). It covers the full
trading lifecycle (quoting, signing, placing, tracking, and cancelling orders)
and exposes lower-level building blocks so you can compose your own flows:
conditional orders (TWAP, stop-loss), on-chain reads via `eth_call`, subgraph
queries, EIP-2612 permits, ethflow, bridging, CowShed hooks, flash loans and
Weiroll scripts.

The SDK is split into a layered workspace of 25 `cow-*` crates with strict
layer boundaries (L0 primitives → L6 façade). Depend on `cow-rs` for the
batteries-included façade, or pick individual `cow-*` crates for tighter
tree-shaking and smaller build graphs. See [Architecture](#architecture).

It runs natively and compiles to WebAssembly for use in the browser.

## Table of Contents

- [Overview](#overview)
- [Supported Chains](#supported-chains)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Workspace Crates](#workspace-crates)
- [Examples](#examples)
- [Feature Flags](#feature-flags)
- [WebAssembly](#webassembly)
- [Architecture](#architecture)
- [Development](#development)
- [Contributing](#contributing)

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
- **Cross-chain bridging** via Across, Bungee, and NEAR Intents — including
  non-EVM destinations (BTC / SOL) with attestor-signed quotes and full
  `cow-shed` post-hook signing.

## Supported Chains

| Chain        | Chain ID   | Network |
| ------------ | ---------- | ------- |
| Ethereum     | `1`        | Mainnet |
| Gnosis Chain | `100`      | Mainnet |
| Arbitrum One | `42161`    | Mainnet |
| Base         | `8453`     | Mainnet |
| Polygon      | `137`      | Mainnet |
| Avalanche    | `43114`    | Mainnet |
| BNB Chain    | `56`       | Mainnet |
| Sepolia      | `11155111` | Testnet |

See [`SupportedChainId`](https://docs.rs/cow-chains/latest/cow_chains/enum.SupportedChainId.html)
for the authoritative list.

## Installation

The easiest way is to depend on the `cow-rs` façade — it re-exports every
layered crate under one entry point:

```toml
[dependencies]
cow-rs = "0.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
alloy-primitives = "1"
```

If you only need part of the SDK you can pick the individual crates
directly. For example, a frontend that just needs to sign orders and talk
to the orderbook:

```toml
[dependencies]
cow-chains = "0.3"
cow-orderbook = "0.3"
cow-signing = "0.3"
cow-trading = "0.3"
cow-types = "0.3"
```

See [Workspace Crates](#workspace-crates) for the full list.

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

## Workspace Crates

The workspace is a layered DAG: a crate on layer `N` may only depend on
crates on strictly lower layers (enforced in CI via
`scripts/check-workspace-layers.py`). The façade `cow-rs` sits at the top
and re-exports everything.

| Layer | Crate                | Purpose                                                           |
| ----- | -------------------- | ----------------------------------------------------------------- |
| L0    | `cow-errors`         | Unified `CowError` — workspace infrastructure                     |
| L0    | `cow-primitives`     | Numeric constants, zero addresses                                 |
| L0    | `cow-chains`         | Chain IDs, contract addresses, canonical endpoints                |
| L1    | `cow-types`          | Protocol types (`OrderKind`, `SigningScheme`, `UnsignedOrder`, …) |
| L2    | `cow-signing`        | EIP-712 signing, `OrderUid` computation                           |
| L2    | `cow-app-data`       | Order metadata schema and `keccak256` hashing                     |
| L2    | `cow-permit`         | EIP-2612 permit signing and hook building                         |
| L2    | `cow-erc20`          | ERC-20 and EIP-2612 calldata builders                             |
| L2    | `cow-ethflow`        | Native ETH order flow                                             |
| L2    | `cow-weiroll`        | Weiroll scripting for batch operations                            |
| L2    | `cow-shed`           | CowShed hook framework                                            |
| L2    | `cow-settlement`     | Settlement encoder, simulator, vault helpers                      |
| L3    | `cow-http`           | HTTP transport: rate limiter, retry policy                        |
| L4    | `cow-orderbook`      | Orderbook REST API client (OpenAPI-generated)                     |
| L4    | `cow-subgraph`       | Historical trading data via GraphQL                               |
| L4    | `cow-onchain`        | JSON-RPC `eth_call` reader                                        |
| L5    | `cow-trading`        | High-level `TradingSdk` and fee-breakdown types                   |
| L5    | `cow-composable`     | Conditional orders (TWAP, GAT, stop-loss) and Merkle multiplexer  |
| L5    | `cow-bridging`       | Cross-chain bridging                                              |
| L5    | `cow-flash-loans`    | Flash loan integration                                            |
| L6    | `cow-browser-wallet` | EIP-1193 browser wallet adapter and WASM bindings                 |
| L6    | `cow-rs`             | Façade re-exporting every layered crate                           |

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

# Cross-chain bridging (v0.3+):
cargo run -p examples-native --example bridging_quote
cargo run -p examples-native --example bridging_cross_chain_post
cargo run -p examples-native --example bridging_near_sol
cargo run -p examples-native --example bridging_near_btc
```

| Folder         | What it shows                                                                 |
| -------------- | ----------------------------------------------------------------------------- |
| `orders/`      | quote, swap, limit, signing, status, cancellation                             |
| `composable/`  | TWAP and stop-loss conditional orders                                         |
| `config/`      | picking a chain and reading its constants                                     |
| `onchain/`     | reading balances and allowances via `eth_call`                                |
| `subgraph/`    | historical data queries                                                       |
| `permit/`      | EIP-2612 approve-by-signature                                                 |
| `cow_shed/`    | pre- and post-trade hooks                                                     |
| `flash_loans/` | borrowing inside a settlement                                                 |
| `bridging/`    | cross-chain intents: EVM↔EVM (Across / Bungee) + EVM↔BTC / SOL (NEAR Intents) |
| `weiroll/`     | batched call scripting                                                        |
| `erc20/`       | calldata encoding helpers                                                     |
| `app_data/`    | building and hashing order metadata                                           |
| `ethflow/`     | trading native ETH                                                            |

A separate WebAssembly example lives in [`examples/wasm`](./examples/wasm).

## Feature Flags

| Flag     | Default | Description                                              |
| -------- | :-----: | -------------------------------------------------------- |
| `native` |   on    | Native (tokio + reqwest) HTTP transport                  |
| `wasm`   |   off   | `wasm-bindgen` glue, browser wallet bridge, fetch client |

Enable WASM builds with:

```toml
cow-rs = { version = "0.3", default-features = false, features = ["wasm"] }
```

## WebAssembly

`cow-rs` compiles to `wasm32-unknown-unknown` and exposes a browser-wallet
bridge so the signing step can be delegated to an injected provider
(MetaMask, Rabby, etc.) instead of a raw private key. See
[`examples/wasm`](./examples/wasm) for an end-to-end setup.

## Architecture

Multi-crate layered workspace. The SDK is split into 25 `cow-*` crates
organised as a strict DAG (Layer 0 → Layer 6), plus the `cow-rs` façade
that re-exports every layer:

```
cow-rs/
├── crates/
│   ├── errors/          # L0 — unified CowError
│   ├── primitives/      # L0 — numeric constants, zero addresses
│   ├── chains/          # L0 — chain IDs, contracts, endpoints
│   ├── types/           # L1 — OrderKind, SigningScheme, UnsignedOrder, …
│   ├── signing/         # L2 — EIP-712 signing, OrderUid
│   ├── app-data/        # L2 — metadata schema + keccak256 hashing
│   ├── permit/          # L2 — EIP-2612 permit signing
│   ├── erc20/           # L2 — ERC-20 calldata
│   ├── ethflow/         # L2 — native ETH order flow
│   ├── weiroll/         # L2 — weiroll script builder
│   ├── cow-shed/        # L2 — CowShed hook framework
│   ├── settlement/      # L2 — encoder, simulator, vault helpers
│   ├── http/            # L3 — rate limiter, retry policy
│   ├── orderbook/       # L4 — OpenAPI-generated REST client
│   ├── subgraph/        # L4 — GraphQL client
│   ├── onchain/         # L4 — eth_call reader
│   ├── trading/         # L5 — high-level TradingSdk
│   ├── composable/      # L5 — TWAP, stop-loss, Merkle multiplexer
│   ├── bridging/        # L5 — cross-chain bridging
│   ├── flash-loans/     # L5 — flash loan integration
│   ├── browser-wallet/  # L6 — EIP-1193 adapter + WASM bindings
│   ├── cow-rs/          # L6 — façade re-exporting every layer
│   └── …                # infra crates: graph, ipfs, testing, contracts-abi
├── examples/
│   ├── native/          # cargo examples (tokio)
│   └── wasm/            # browser example
├── fuzz/                # cargo-fuzz targets
├── scripts/             # spec fetchers, layer DAG checker, conformance tools
└── docs/adr/            # architecture decision records
```

Layer rules are enforced in CI via `scripts/check-workspace-layers.py`:
a crate at layer `N` may only depend on crates at strictly lower layers,
and two crates on the same layer may not depend on each other. Design
notes and trade-offs are recorded as ADRs in [`docs/adr`](./docs/adr).

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
byte-for-byte parity on signing and encoding. See
[`crates/cow-rs/specs/`](./crates/cow-rs/specs).

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
