# cow-rs WASM Demo

Browser demo showing the cow-rs SDK running as WebAssembly.

## Prerequisites

```bash
cargo install wasm-pack
```

## Build

```bash
cd examples/wasm
wasm-pack build --target web
```

## Run

Serve the directory with any HTTP server:

```bash
python3 -m http.server 8080
# or
npx serve .
```

Then open <http://localhost:8080> in your browser.

## What it demonstrates

- **Configuration** — list supported chains, compute domain separators, look up contract addresses
- **App Data** — hash and encode `AppDataDoc` documents, get CID info
- **Order Hashing** — compute EIP-712 order struct hashes
- **Live API** — fetch quotes and subgraph totals from CoW Protocol (real network requests from the browser)

All computations run entirely in WASM. Network requests use the browser's native `fetch` API via `reqwest`'s WASM backend.
