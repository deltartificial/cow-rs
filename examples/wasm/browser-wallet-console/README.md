# cow-rs Browser Wallet Console

Browser demo combining mock (deterministic) and injected (MetaMask) wallet interactions with the CoW Protocol SDK via WebAssembly.

## Build

```bash
cd examples/wasm/browser-wallet-console
wasm-pack build --target web
```

## Serve

```bash
# Any HTTP server works. Examples:
python3 -m http.server 8080
# or
npx serve .
```

Then open `http://localhost:8080` in a browser.

## Features

- **Mock wallet**: deterministic signing with the Hardhat #0 test key (no extension needed)
- **Injected wallet**: detects and uses `window.ethereum` (MetaMask, etc.)
- **Sample data**: chain-aware token addresses and order templates
- **Live API**: fetches real quotes from the CoW Protocol orderbook
