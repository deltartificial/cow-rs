# Changelog

All notable changes to this project will be documented in this file.

## [0.5.1] - 2026-04-26

### Features

- _(app-data)_ Accept base32 multibase CIDs in `parse_cid` and `cid_to_appdata_hex` (#85) ([`2f0b102`](https://github.com/deltartificial/cow-rs/commit/2f0b102))

### Bug fixes

- _(signing)_ Align EIP-712 domain name and Order type hash with TS SDK (#86) ([`4429e55`](https://github.com/deltartificial/cow-rs/commit/4429e55))
- _(ethflow)_ Encode `quoteId` inside the `createOrder` struct argument (#87) ([`62a7352`](https://github.com/deltartificial/cow-rs/commit/62a7352))
- _(bridging)_ Unbreak doc link in `bungee_approve_and_bridge_v1_addresses` (#98) ([`23f4dbf`](https://github.com/deltartificial/cow-rs/commit/23f4dbf))

### Testing

This release significantly improves coverage across the workspace.
Aggregate workspace coverage moved from ~96.2 % to **99.95 %** on lcov
DA lines (excluding the wasm-only `browser-wallet` crate, which is
exercised in a browser runtime). PRs: #81, #82, #83, #84, #88, #89,
#90, #91, #92, #93, #94, #95, #96, #97.

### Refactor

Several internal refactors keep behaviour identical to 0.5.0 but make
defensive code paths reachable from coverage tooling and tighten loud
failure on hardcoded misconfiguration:

- `decode_order_flags` and `decode_signing_scheme` in `cow-types::flags` no longer use `unreachable!()` after a bit-mask; the binary case becomes `if/else` and the four-case becomes a constant array index. Public signatures are unchanged (`decode_signing_scheme` is now `pub const fn`).
- `bungee_approve_and_bridge_v1_addresses` panics with a descriptive message if the hardcoded address constant fails to parse, instead of silently returning an empty `HashMap`.
- A new `#[cfg_attr(coverage_nightly, coverage(off))]` attribute is applied to a handful of test fixtures and trivial defensive helpers across `cow-bridging`, `cow-app-data`, `cow-rs`, `cow-signing`, and `cow-orderbook`. The attribute is gated on the `coverage_nightly` cfg that `cargo-llvm-cov` sets, so stable builds are unaffected.

## [0.5.0] - 2026-04-22

### Documentation

- Bump README install snippets to 0.4 (#78) ([`06b1ca2`](https://github.com/deltartificial/cow-rs/commit/06b1ca296d7f635cc9c003cc748bad031e4594b2))

### Refactor

- _(bridging)_ [**breaking**] Add TokenAddress enum for non-EVM destinations (#79) ([`effd1cd`](https://github.com/deltartificial/cow-rs/commit/effd1cdac4d27e0ef2f6cad11fd10d977fcba6de))

## [0.4.0] - 2026-04-21

### Features

- _(bridging)_ Add SigningStepManager sync helpers (#72) ([`fc659d7`](https://github.com/deltartificial/cow-rs/commit/fc659d7e2487077cd6d864c58fdb4cfaecdc6830))
- _(bridging)_ TTL eviction on NearDepositCache (#74) ([`823c8db`](https://github.com/deltartificial/cow-rs/commit/823c8db7d613380d3591c3a401d0309148c0bd48))
- _(examples)_ Add WASM bridging console (#76) ([`88fe63d`](https://github.com/deltartificial/cow-rs/commit/88fe63dbb91ffcc141077d73b5cf03cada47dd3b))

### Miscellaneous

- _(release)_ Bump workspace to v0.4.0 (#77) ([`0e7cf59`](https://github.com/deltartificial/cow-rs/commit/0e7cf59d9af5d63d68a3cc8e8e6b3c945e3ad52e))

### Refactor

- _(bridging)_ [**breaking**] Drop deprecated create_post_swap_order_from_quote stub (#71) ([`e6e5240`](https://github.com/deltartificial/cow-rs/commit/e6e5240c92eea00265312e7d0d406236c323bf10))

### Testing

- _(chains)_ Assert Plasma chain_id + well-known EVM ids match TS SDK (#73) ([`fce5049`](https://github.com/deltartificial/cow-rs/commit/fce5049d2c963284a2b40382b1fd2eb186a5009f))
- _(bridging)_ NEAR attestation conformance vectors (#75) ([`cbc82a3`](https://github.com/deltartificial/cow-rs/commit/cbc82a3725962d93f634d88509ac2f29037f964e))

## [0.3.0] - 2026-04-21

### Features

- _(bridging)_ [**breaking**] Enrich BridgeProvider trait with info/networks/status (#56) ([`959383b`](https://github.com/deltartificial/cow-rs/commit/959383b8a14dcbaf70795f4ed67c78540630dc33))
- _(bridging)_ Split HookBridgeProvider and ReceiverAccountBridgeProvider (#57) ([`ca98714`](https://github.com/deltartificial/cow-rs/commit/ca987141c40e11c683fe66ee33e5d6668d239f0a))
- _(cow-shed)_ Add EIP-712 sign_hook for CoWShed hook bundles (#58) ([`6bda283`](https://github.com/deltartificial/cow-rs/commit/6bda2831c441e9cbbb8bf25267b1d0e810d4f187))
- _(bridging)_ Add AcrossBridgeProvider implementing hook sub-trait (#59) ([`6a65d76`](https://github.com/deltartificial/cow-rs/commit/6a65d7688aaac6f8510d27b5b741579dc2a24cf3))
- _(bridging)_ Upgrade BungeeProvider to enriched trait (#60) ([`7068b25`](https://github.com/deltartificial/cow-rs/commit/7068b252474d15cc81732d940ef54abe72e255e2))
- _(bridging)_ Implement get_intermediate_swap_result with cow-sdk#852 metadata fix (#61) ([`7193623`](https://github.com/deltartificial/cow-rs/commit/7193623c8d2fbb344b322612f63aa9a936be4497))
- _(bridging)_ Implement get_quote_with_bridge with SwapQuoter for TradingSdk (#62) ([`740e43f`](https://github.com/deltartificial/cow-rs/commit/740e43fee5ebe68143936eb24d59cea0132d1d00))
- _(bridging)_ [**breaking**] Real get_bridge_signed_hook + SigningStepManager + post_cross_chain_order (#63) ([`a619ed7`](https://github.com/deltartificial/cow-rs/commit/a619ed75e3733b403ac73b95d3eff3957dc475d5))
- _(bridging)_ NearIntentsApi HTTP client + wire types (#65) ([`150a33f`](https://github.com/deltartificial/cow-rs/commit/150a33f27e39feaff7a30af2cec95332ba50e81a))
- _(bridging)_ Add NearIntentsBridgeProvider with attestation verification (#66) ([`b0315b4`](https://github.com/deltartificial/cow-rs/commit/b0315b4bd033df5f610bcb014cd0935c8ed679ed))
- _(bridging)_ Add deposit-address cache + enable non-EVM destinations via ZERO sentinel (#68) ([`6c54ca1`](https://github.com/deltartificial/cow-rs/commit/6c54ca13fb1a7dee7bf33e6ce3d2704cefc5b188))
- _(examples)_ Add 3 bridging examples (cross-chain post + NEAR SOL/BTC) (#69) ([`4bba62d`](https://github.com/deltartificial/cow-rs/commit/4bba62d85aa541d4271428865fd300d01900bdca))

### Miscellaneous

- _(release)_ Bump workspace to v0.3.0 (#70) ([`ffdea90`](https://github.com/deltartificial/cow-rs/commit/ffdea90ed1dc159df5c98c75de56489d367f8169))

### Testing

- _(bridging)_ Cover post_cross_chain_order error paths and advanced settings (#64) ([`d792212`](https://github.com/deltartificial/cow-rs/commit/d792212c40fb5afdd0ba2caafde91fdbce9e62e3))
- _(bridging)_ Raise near_intents provider coverage to 100% lines (#67) ([`cdd2654`](https://github.com/deltartificial/cow-rs/commit/cdd2654baff2e4614f9cdc5766275ae391f68730))

## [0.2.0] - 2026-04-19

### Bug Fixes

- _(ci)_ Green all post-split workflows (#49) ([`74dedb4`](https://github.com/deltartificial/cow-rs/commit/74dedb4cfb5298c65114eb7c04566c12f2d295ec))
- _(ci)_ Reorder orderbook build/dev-dependencies to satisfy cargo-sort (#52) ([`895f9f7`](https://github.com/deltartificial/cow-rs/commit/895f9f77e2da91f056f0839904875afa0db0c9c7))

### Documentation

- _(changelog)_ Regenerate for v0.1.1 via git-cliff ([`ba26e32`](https://github.com/deltartificial/cow-rs/commit/ba26e3213e0efc5c06949b43e2c13e5d26cc9dd8))
- _(changelog)_ Use underscore italics to match dprint markdown style ([`2d8f6ba`](https://github.com/deltartificial/cow-rs/commit/2d8f6ba9fa03e9167894c3c98ca3f4ebff7842d6))
- Refresh README and changelog for multi-crate workspace (#51) ([`81845b1`](https://github.com/deltartificial/cow-rs/commit/81845b121d4d642d4d3f3429b2be4077174f8003))

### Features

- [**breaking**] Sync TS SDK parity through cow-sdk@a5207e0d (#54) ([`c5c0c5c`](https://github.com/deltartificial/cow-rs/commit/c5c0c5cd7cc0487529bec7103ec0b6a457f11302))

### Miscellaneous

- _(release)_ Publish workspace 0.1.2 to crates.io (#53) ([`4e79ff4`](https://github.com/deltartificial/cow-rs/commit/4e79ff405efe38775cf46c7494aad7b8a4d49b8c))
- _(release)_ Bump workspace to v0.2.0 (#55) ([`3003670`](https://github.com/deltartificial/cow-rs/commit/3003670a58f695fc84cd4ef8d34de251aaa2559a))

### Refactor

- _(workspace)_ [**breaking**] Split monolithic cow-rs into 26 layered crates (#48) ([`3c2844a`](https://github.com/deltartificial/cow-rs/commit/3c2844a3185432bece303a228bc310024525279a))
- _(workspace)_ Finish split debt and modernize examples (#50) ([`c9b8971`](https://github.com/deltartificial/cow-rs/commit/c9b8971620e36b69504b6bc41241c00c2ff8c262))

## [0.1.1] - 2026-04-14

### Bug Fixes

- Resolve all CI lint and test failures (#2) ([`68b0226`](https://github.com/deltartificial/cow-rs/commit/68b02260144240b6eff56d26c3544be6d21f48f7))
- Resolve all remaining CI failures (dprint, sort, udeps, miri, minimal-versions) ([`bcfd5bc`](https://github.com/deltartificial/cow-rs/commit/bcfd5bc98747cf5d0d6774f765ed1aebae03da72))
- Resolve dprint, cargo-sort, udeps, and miri CI failures ([`4da2459`](https://github.com/deltartificial/cow-rs/commit/4da2459da395384addbcbed7483c5a64580c7b90))
- Resolve remaining miri and minimal-versions CI failures ([`ebfacd9`](https://github.com/deltartificial/cow-rs/commit/ebfacd9f35fa4e13636ad22bb66ba15eadf96d00))
- Skip properties tests under miri (proptest uses getcwd) ([`692db04`](https://github.com/deltartificial/cow-rs/commit/692db049334eb65ab4e81356a6a2e18c025de0cd))
- Add syn as direct dep to fix minimal-versions resolution ([`3bec563`](https://github.com/deltartificial/cow-rs/commit/3bec563f491b83dd43f9fd1af7e2caa02efa73fb))
- Resolve miri, deny, and minimal-versions CI failures ([`257b92a`](https://github.com/deltartificial/cow-rs/commit/257b92a6cfe8af4dbbd843f74027890d25560fc7))
- _(codegen)_ Resolve lint/doc/deny failures and align Referrer with upstream ([`0db79d3`](https://github.com/deltartificial/cow-rs/commit/0db79d34d9bea41f9278bd68c5fddc233849c08c))
- _(ci)_ Gate tokio usage on `native` feature and pin alloy-consensus minimum (#3) ([`deab597`](https://github.com/deltartificial/cow-rs/commit/deab597ecabfebdcea8c751697950f72195dfc8f))
- _(deps)_ Pin tracing ≥0.1.40 for minimal-versions compatibility (#4) ([`70e6fd6`](https://github.com/deltartificial/cow-rs/commit/70e6fd62ce9f2aebab934095209f573f43482ef3))
- _(ci)_ Skip hashbrown 0.16 duplicate in cargo-deny (#5) ([`389ebb8`](https://github.com/deltartificial/cow-rs/commit/389ebb894ac2a05ba2ffadf24841be7d92a07340))
- _(ci)_ Pin transitive deps for minimal-versions and fix dprint format (#6) ([`40736d7`](https://github.com/deltartificial/cow-rs/commit/40736d7269bb36c5cd7c78877381fed2cc6fedee))
- _(ci)_ Sort workspace and crate deps alphabetically (#7) ([`7b7eba8`](https://github.com/deltartificial/cow-rs/commit/7b7eba8ac1b91834f61ba6924ca11da88d79a63d))
- _(ci)_ Pin ahash, ref-cast, alloy-rpc-types-eth for minimal-versions (#8) ([`57e7d35`](https://github.com/deltartificial/cow-rs/commit/57e7d358d16794f06105a8203a7a8756ee460153))
- _(ci)_ Pin alloy-signer ≥1.5 for minimal-versions (#9) ([`f7f6d83`](https://github.com/deltartificial/cow-rs/commit/f7f6d83a766660e1d70d51dbc85245a3bd87c07f))
- _(ci)_ Bump alloy-signer pin to ≥1.8.3 for minimal-versions (#10) ([`2b70317`](https://github.com/deltartificial/cow-rs/commit/2b70317e7d388b5c06040219acf986a76028e8dd))
- _(ci)_ Resolve clippy and fmt errors in test code (#23) ([`73943dd`](https://github.com/deltartificial/cow-rs/commit/73943ddf68cf9d217318da1510b68f9c496a06f1))
- _(ci)_ Use Arc::clone instead of .clone() on ref-counted pointer (#27) ([`f0ab089`](https://github.com/deltartificial/cow-rs/commit/f0ab08931f07fb6e986eb594e2bb8869ddb357ed))
- _(ci)_ Fix doc_markdown backticks and fmt in wasm.rs (#30) ([`c2e798c`](https://github.com/deltartificial/cow-rs/commit/c2e798c456177254f55bbf33bea79b5845fe8024))
- _(ci)_ Resolve all clippy lints in traits, browser_wallet, conformance (#38) ([`7ced2db`](https://github.com/deltartificial/cow-rs/commit/7ced2db401d6528a2522ca95a91a178bc16fec54))
- _(ci)_ Unused import, must_use let, redundant clone (#41) ([`e2aca71`](https://github.com/deltartificial/cow-rs/commit/e2aca7176eae053f102057be10dfe006a24ac222))
- _(ci)_ Disallowed drop, redundant clone, manual_string_new (#43) ([`dc860e7`](https://github.com/deltartificial/cow-rs/commit/dc860e7d1174ad83c3f6ab594d3a0adee7194ab3))
- _(ci)_ Use Path::join for Windows-compatible conformance fixture paths (#44) ([`308b0d8`](https://github.com/deltartificial/cow-rs/commit/308b0d8bcce1b1bced130c06ccb118ad20230d21))
- _(ci)_ Rename shadowed path variable in conformance loader (#45) ([`3451ef5`](https://github.com/deltartificial/cow-rs/commit/3451ef5b3bc30b37a28423f9e40338a4379c047b))
- _(app-data)_ Stop double-hashing appDataHex in appdata_hex_to_cid (#47) ([`3c474ac`](https://github.com/deltartificial/cow-rs/commit/3c474ac78d6195d02b1c4ae729b06eadf7f87951))

### CI

- _(readme)_ Add shields.io badges and Codecov coverage workflow (#20) ([`6643c31`](https://github.com/deltartificial/cow-rs/commit/6643c31b7e20addc54b7f211f5e6f4e5f5a00981))
- Add CODECOV_TOKEN to coverage upload (#24) ([`764197d`](https://github.com/deltartificial/cow-rs/commit/764197d12d8261477efc35d0e0fff675b3b45ab8))

### Documentation

- _(readme)_ Remove MSRV badge (#21) ([`94e6119`](https://github.com/deltartificial/cow-rs/commit/94e6119bf8285cf92bd740c5c282aeac0e9586f4))
- _(readme)_ Add blank line between badges and description (#22) ([`524eba8`](https://github.com/deltartificial/cow-rs/commit/524eba837667779aad5a4f36431b9e5f9dbecda5))
- _(changelog)_ Add unreleased section with PRs #3–#27 (#25) ([`154f8ad`](https://github.com/deltartificial/cow-rs/commit/154f8ad2426e3569ac9204cfadd41c6edd6ba7c3))
- _(examples)_ Restructure into thematic folders and add 8 new examples (#46) ([`db139ac`](https://github.com/deltartificial/cow-rs/commit/db139ac2955db8e1605b6c414270819281f02a08))
- _(readme)_ Refresh with full overview, examples guide, and feature flags ([`e9d00f2`](https://github.com/deltartificial/cow-rs/commit/e9d00f2c33ad03425be3ce41448cb44d2fe6a6ce))

### Features

- Initial CoW Protocol Rust SDK (#1) ([`fc15154`](https://github.com/deltartificial/cow-rs/commit/fc1515420ea9f191c1bd3181444d1331111e3c77))
- Add build-time OpenAPI codegen, GraphQL/AppData schema validation ([`0f2d8ee`](https://github.com/deltartificial/cow-rs/commit/0f2d8ee311c2a3ef5ff8b54844701df03d26b681))
- _(order-book)_ Add wire-compat drift tests between hand + generated types ([`3a5a0a3`](https://github.com/deltartificial/cow-rs/commit/3a5a0a362eb1b3e326885d4aac86e3c031fc099a))
- _(app-data)_ Expose runtime JSON Schema validation ([`6e5cc10`](https://github.com/deltartificial/cow-rs/commit/6e5cc10282e415cec1db58d1bb3d5312c17ac8f4))
- _(app-data)_ Multi-version JSON Schema dispatch ([`1823eab`](https://github.com/deltartificial/cow-rs/commit/1823eabf222563e9f4e7d2c77c33a4aa7cee2361))
- _(order-book)_ Rate limiting and retry policy on OrderBookApi ([`1d174c8`](https://github.com/deltartificial/cow-rs/commit/1d174c8496906aec36b2e530992692c63485ae83))
- _(subgraph)_ Rate limiting and retry policy on SubgraphApi ([`8b0a73d`](https://github.com/deltartificial/cow-rs/commit/8b0a73d5e2819d1b5ec927233f72211576639d87))
- _(order-book)_ Partner API header support ([`1e47dbe`](https://github.com/deltartificial/cow-rs/commit/1e47dbe662cb2b97a47fd043aced3d68ff6d6585))
- _(app-data)_ Referrer enum + register v1.14.0 schema ([`c2ce741`](https://github.com/deltartificial/cow-rs/commit/c2ce741b98440d8d8caccb8738c94d29035bfbd3))
- _(fuzz)_ Add 5 fuzz targets covering core SDK parsers (#13) ([`7ee11fb`](https://github.com/deltartificial/cow-rs/commit/7ee11fb9840047ce87ccb774b380e46a2b699ff1))
- _(fuzz)_ Complete coverage with 8 additional fuzz targets (#14) ([`6b77300`](https://github.com/deltartificial/cow-rs/commit/6b77300d5e6f4a48b323fea330895cda1df8c6b3))
- _(examples)_ Add browser-wallet-console WASM example (#28) ([`a549a35`](https://github.com/deltartificial/cow-rs/commit/a549a355989502c1e23820967096c82a5de5df34))
- _(wasm)_ Add signOrderWithBrowserWallet for EIP-1193 wallet signing (#29) ([`fd2352c`](https://github.com/deltartificial/cow-rs/commit/fd2352c921330069fb44b1ebf0747f4d4904f078))
- Add conformance test framework and maintainer CLI (#31) ([`8ba3f07`](https://github.com/deltartificial/cow-rs/commit/8ba3f0757fd9f29431f62bbd3ccf1d38561e1816))
- _(settlement)_ Add SettlementEncoder, vault roles, and state readers (#32) ([`6271ab9`](https://github.com/deltartificial/cow-rs/commit/6271ab9b11350fe8cc4d2627a5fd8e43943353c0))
- Add injectable trait abstractions (OrderbookClient, CowSigner, RpcProvider) (#33) ([`b39d74e`](https://github.com/deltartificial/cow-rs/commit/b39d74ef898aabae5312ef293de36c548de7f6af))
- BrowserWallet SDK struct + orderbook/trading conformance fixtures (#34) ([`024b535`](https://github.com/deltartificial/cow-rs/commit/024b535f98aa08f83ca707eb7b8753893dc0eee6))
- Domain separator override, IpfsClient trait, ADRs (#35) ([`fb346ba`](https://github.com/deltartificial/cow-rs/commit/fb346ba2d1d5eadc03aff9ae730280d50fea4abf))
- _(browser-wallet)_ Session management, events, chain switching, stateful mock (#36) ([`71503aa`](https://github.com/deltartificial/cow-rs/commit/71503aad069d52a648aeef140eee0e9bd646dda9))
- _(settlement)_ Add TradeSimulator and order refund utilities (#37) ([`41c7ed4`](https://github.com/deltartificial/cow-rs/commit/41c7ed417e506bc91544578d8a11cee9f96d8cca))

### Miscellaneous

- _(codegen)_ Pin upstream commit in orderbook spec provenance ([`62c3a13`](https://github.com/deltartificial/cow-rs/commit/62c3a13135f945b8f921e3694692e0904a1408da))
- _(ci)_ Remove minimal-versions job and clean up version pins (#11) ([`bd6ca1a`](https://github.com/deltartificial/cow-rs/commit/bd6ca1ad97f4a2cdbf15263633d1c94750e6c537))
- _(ci)_ Remove miri job — hangs and times out on this workspace (#12) ([`999a63b`](https://github.com/deltartificial/cow-rs/commit/999a63bc9cd0c34a42619b9a34cbe40c92a1380d))
- Move conformance/ into scripts/conformance/ (#39) ([`2169429`](https://github.com/deltartificial/cow-rs/commit/2169429d49c4f8067ab84999fad422c5aa1346a2))
- Bump workspace version to 0.1.0 for initial crates.io release ([`06f66fd`](https://github.com/deltartificial/cow-rs/commit/06f66fd4383b233b0c448beff55125e0916ef680))
- _(release)_ Relocate specs into crate and add publish metadata ([`36c6e8f`](https://github.com/deltartificial/cow-rs/commit/36c6e8f8df497a1d6b44d79686aab374fba436c9))
- _(release)_ 0.1.1 ([`b6c2d82`](https://github.com/deltartificial/cow-rs/commit/b6c2d825e46e415cc4eacd67f943a8ac6414e986))

### Refactor

- _(subgraph)_ Replace lying schema tests with a real query linter ([`250e07f`](https://github.com/deltartificial/cow-rs/commit/250e07fa05d7b6c41867c33a5ac58ec249ee2032))

### Testing

- Add unit tests for types, contracts, params, calldata (42% → 48%) (#15) ([`790bb11`](https://github.com/deltartificial/cow-rs/commit/790bb1106a1949196d8c52e66923e37f04e6cc2f))
- Comprehensive unit tests across SDK (42% → 82% coverage) (#16) ([`7ac98a0`](https://github.com/deltartificial/cow-rs/commit/7ac98a07331f5b51840c2774a0e97f2453716fa8))
- Wiremock API tests + sync coverage push (82% → 85%) (#17) ([`e79eb0f`](https://github.com/deltartificial/cow-rs/commit/e79eb0feedc52499d4d3bda60ce5af05e969f2ef))
- Bridging + IPFS wiremock tests (85% → 90% coverage) (#18) ([`582430e`](https://github.com/deltartificial/cow-rs/commit/582430ed21fedbdec046acb37f5ec2a0a8764820))
- Push coverage to 92% with expanded wiremock tests (#19) ([`4af871b`](https://github.com/deltartificial/cow-rs/commit/4af871bc0351399b02a01fb56ef6d12e8bdd3d5c))
- Push coverage to 96% — trading/sdk, order_book, bridging, onchain (#26) ([`c90351d`](https://github.com/deltartificial/cow-rs/commit/c90351d9c8f6b06f5c6f3c94d239c533a4097818))
- Push coverage to 97% — async mocks + sync edge cases (#40) ([`566f6aa`](https://github.com/deltartificial/cow-rs/commit/566f6aa768e989f5fcb5046045eb17e85edfa28d))
- Push coverage to 97% + add codecov.yml (#42) ([`fd11b6e`](https://github.com/deltartificial/cow-rs/commit/fd11b6efb5308c030da4539a37b89c1eb3314320))

### Style

- _(readme)_ Apply dprint table formatting ([`4a0ff3a`](https://github.com/deltartificial/cow-rs/commit/4a0ff3a5550e87c8ab7aaa464fa453487ab30ab8))
