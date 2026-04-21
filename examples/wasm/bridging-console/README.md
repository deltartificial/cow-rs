# cow-rs Bridging Console

Browser demo of the cow-rs bridging helpers running 100% in WebAssembly — focused on deterministic NEAR Intents primitives (provider metadata, supported-chains table, canonical quote hashing, attestation recovery).

Live-API quotes are out of scope for this minimal demo (they need a CORS-proxied NEAR endpoint); everything in this console is pure-compute.

## Build

```bash
cd examples/wasm/bridging-console
wasm-pack build --target web
```

## Serve

Any static HTTP server works:

```bash
python3 -m http.server 8080
# or
npx serve .
```

Then open `http://localhost:8080`.

## What's exposed

| JS binding                                        | Purpose                                                                                    |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `nearIntentsInfo()`                               | Returns `{ name, dapp_id, kind, default_validity_secs }`                                   |
| `supportedChains()`                               | Returns the 11 NEAR-supported chains as `[{ chain_id, key }]`                              |
| `canonicalQuoteHash(amount?)`                     | SHA-256 of a canonical quote payload — useful for diffing against the TS SDK byte-for-byte |
| `verifyNearAttestation(deposit, hash, signature)` | Wraps `recover_attestation` — returns the recovered EVM signer address                     |

## What's not included

- Live API calls — `NearIntentsApi::get_quote` requires a CORS-proxied NEAR endpoint.
- EIP-712 signing — covered by `../browser-wallet-console`.
- Order posting — covered by `../browser-wallet-console`.

## Structure

```text
bridging-console/
├── Cargo.toml         # wasm-bindgen + cow-rs with `wasm` feature
├── src/lib.rs         # BridgingConsole struct + JS exports
├── index.html         # Minimal UI
└── README.md
```
