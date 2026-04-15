# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2026-04-14

### Bug Fixes

- Resolve all CI lint and test failures (#2) ([`68b0226`](https://github.com/deltartificial/cow-rs/commit/68b02260144240b6eff56d26c3544be6d21f48f7))
- Resolve all remaining CI failures (dprint, sort, udeps, miri, minimal-versions) ([`bcfd5bc`](https://github.com/deltartificial/cow-rs/commit/bcfd5bc98747cf5d0d6774f765ed1aebae03da72))
- Resolve dprint, cargo-sort, udeps, and miri CI failures ([`4da2459`](https://github.com/deltartificial/cow-rs/commit/4da2459da395384addbcbed7483c5a64580c7b90))
- Resolve remaining miri and minimal-versions CI failures ([`ebfacd9`](https://github.com/deltartificial/cow-rs/commit/ebfacd9f35fa4e13636ad22bb66ba15eadf96d00))
- Skip properties tests under miri (proptest uses getcwd) ([`692db04`](https://github.com/deltartificial/cow-rs/commit/692db049334eb65ab4e81356a6a2e18c025de0cd))
- Add syn as direct dep to fix minimal-versions resolution ([`3bec563`](https://github.com/deltartificial/cow-rs/commit/3bec563f491b83dd43f9fd1af7e2caa02efa73fb))
- Resolve miri, deny, and minimal-versions CI failures ([`257b92a`](https://github.com/deltartificial/cow-rs/commit/257b92a6cfe8af4dbbd843f74027890d25560fc7))
- *(codegen)* Resolve lint/doc/deny failures and align Referrer with upstream ([`0db79d3`](https://github.com/deltartificial/cow-rs/commit/0db79d34d9bea41f9278bd68c5fddc233849c08c))
- *(ci)* Gate tokio usage on `native` feature and pin alloy-consensus minimum (#3) ([`deab597`](https://github.com/deltartificial/cow-rs/commit/deab597ecabfebdcea8c751697950f72195dfc8f))
- *(deps)* Pin tracing ≥0.1.40 for minimal-versions compatibility (#4) ([`70e6fd6`](https://github.com/deltartificial/cow-rs/commit/70e6fd62ce9f2aebab934095209f573f43482ef3))
- *(ci)* Skip hashbrown 0.16 duplicate in cargo-deny (#5) ([`389ebb8`](https://github.com/deltartificial/cow-rs/commit/389ebb894ac2a05ba2ffadf24841be7d92a07340))
- *(ci)* Pin transitive deps for minimal-versions and fix dprint format (#6) ([`40736d7`](https://github.com/deltartificial/cow-rs/commit/40736d7269bb36c5cd7c78877381fed2cc6fedee))
- *(ci)* Sort workspace and crate deps alphabetically (#7) ([`7b7eba8`](https://github.com/deltartificial/cow-rs/commit/7b7eba8ac1b91834f61ba6924ca11da88d79a63d))
- *(ci)* Pin ahash, ref-cast, alloy-rpc-types-eth for minimal-versions (#8) ([`57e7d35`](https://github.com/deltartificial/cow-rs/commit/57e7d358d16794f06105a8203a7a8756ee460153))
- *(ci)* Pin alloy-signer ≥1.5 for minimal-versions (#9) ([`f7f6d83`](https://github.com/deltartificial/cow-rs/commit/f7f6d83a766660e1d70d51dbc85245a3bd87c07f))
- *(ci)* Bump alloy-signer pin to ≥1.8.3 for minimal-versions (#10) ([`2b70317`](https://github.com/deltartificial/cow-rs/commit/2b70317e7d388b5c06040219acf986a76028e8dd))
- *(ci)* Resolve clippy and fmt errors in test code (#23) ([`73943dd`](https://github.com/deltartificial/cow-rs/commit/73943ddf68cf9d217318da1510b68f9c496a06f1))
- *(ci)* Use Arc::clone instead of .clone() on ref-counted pointer (#27) ([`f0ab089`](https://github.com/deltartificial/cow-rs/commit/f0ab08931f07fb6e986eb594e2bb8869ddb357ed))
- *(ci)* Fix doc_markdown backticks and fmt in wasm.rs (#30) ([`c2e798c`](https://github.com/deltartificial/cow-rs/commit/c2e798c456177254f55bbf33bea79b5845fe8024))
- *(ci)* Resolve all clippy lints in traits, browser_wallet, conformance (#38) ([`7ced2db`](https://github.com/deltartificial/cow-rs/commit/7ced2db401d6528a2522ca95a91a178bc16fec54))
- *(ci)* Unused import, must_use let, redundant clone (#41) ([`e2aca71`](https://github.com/deltartificial/cow-rs/commit/e2aca7176eae053f102057be10dfe006a24ac222))
- *(ci)* Disallowed drop, redundant clone, manual_string_new (#43) ([`dc860e7`](https://github.com/deltartificial/cow-rs/commit/dc860e7d1174ad83c3f6ab594d3a0adee7194ab3))
- *(ci)* Use Path::join for Windows-compatible conformance fixture paths (#44) ([`308b0d8`](https://github.com/deltartificial/cow-rs/commit/308b0d8bcce1b1bced130c06ccb118ad20230d21))
- *(ci)* Rename shadowed path variable in conformance loader (#45) ([`3451ef5`](https://github.com/deltartificial/cow-rs/commit/3451ef5b3bc30b37a28423f9e40338a4379c047b))
- *(app-data)* Stop double-hashing appDataHex in appdata_hex_to_cid (#47) ([`3c474ac`](https://github.com/deltartificial/cow-rs/commit/3c474ac78d6195d02b1c4ae729b06eadf7f87951))

### CI

- *(readme)* Add shields.io badges and Codecov coverage workflow (#20) ([`6643c31`](https://github.com/deltartificial/cow-rs/commit/6643c31b7e20addc54b7f211f5e6f4e5f5a00981))
- Add CODECOV_TOKEN to coverage upload (#24) ([`764197d`](https://github.com/deltartificial/cow-rs/commit/764197d12d8261477efc35d0e0fff675b3b45ab8))

### Documentation

- *(readme)* Remove MSRV badge (#21) ([`94e6119`](https://github.com/deltartificial/cow-rs/commit/94e6119bf8285cf92bd740c5c282aeac0e9586f4))
- *(readme)* Add blank line between badges and description (#22) ([`524eba8`](https://github.com/deltartificial/cow-rs/commit/524eba837667779aad5a4f36431b9e5f9dbecda5))
- *(changelog)* Add unreleased section with PRs #3–#27 (#25) ([`154f8ad`](https://github.com/deltartificial/cow-rs/commit/154f8ad2426e3569ac9204cfadd41c6edd6ba7c3))
- *(examples)* Restructure into thematic folders and add 8 new examples (#46) ([`db139ac`](https://github.com/deltartificial/cow-rs/commit/db139ac2955db8e1605b6c414270819281f02a08))
- *(readme)* Refresh with full overview, examples guide, and feature flags ([`e9d00f2`](https://github.com/deltartificial/cow-rs/commit/e9d00f2c33ad03425be3ce41448cb44d2fe6a6ce))

### Features

- Initial CoW Protocol Rust SDK (#1) ([`fc15154`](https://github.com/deltartificial/cow-rs/commit/fc1515420ea9f191c1bd3181444d1331111e3c77))
- Add build-time OpenAPI codegen, GraphQL/AppData schema validation ([`0f2d8ee`](https://github.com/deltartificial/cow-rs/commit/0f2d8ee311c2a3ef5ff8b54844701df03d26b681))
- *(order-book)* Add wire-compat drift tests between hand + generated types ([`3a5a0a3`](https://github.com/deltartificial/cow-rs/commit/3a5a0a362eb1b3e326885d4aac86e3c031fc099a))
- *(app-data)* Expose runtime JSON Schema validation ([`6e5cc10`](https://github.com/deltartificial/cow-rs/commit/6e5cc10282e415cec1db58d1bb3d5312c17ac8f4))
- *(app-data)* Multi-version JSON Schema dispatch ([`1823eab`](https://github.com/deltartificial/cow-rs/commit/1823eabf222563e9f4e7d2c77c33a4aa7cee2361))
- *(order-book)* Rate limiting and retry policy on OrderBookApi ([`1d174c8`](https://github.com/deltartificial/cow-rs/commit/1d174c8496906aec36b2e530992692c63485ae83))
- *(subgraph)* Rate limiting and retry policy on SubgraphApi ([`8b0a73d`](https://github.com/deltartificial/cow-rs/commit/8b0a73d5e2819d1b5ec927233f72211576639d87))
- *(order-book)* Partner API header support ([`1e47dbe`](https://github.com/deltartificial/cow-rs/commit/1e47dbe662cb2b97a47fd043aced3d68ff6d6585))
- *(app-data)* Referrer enum + register v1.14.0 schema ([`c2ce741`](https://github.com/deltartificial/cow-rs/commit/c2ce741b98440d8d8caccb8738c94d29035bfbd3))
- *(fuzz)* Add 5 fuzz targets covering core SDK parsers (#13) ([`7ee11fb`](https://github.com/deltartificial/cow-rs/commit/7ee11fb9840047ce87ccb774b380e46a2b699ff1))
- *(fuzz)* Complete coverage with 8 additional fuzz targets (#14) ([`6b77300`](https://github.com/deltartificial/cow-rs/commit/6b77300d5e6f4a48b323fea330895cda1df8c6b3))
- *(examples)* Add browser-wallet-console WASM example (#28) ([`a549a35`](https://github.com/deltartificial/cow-rs/commit/a549a355989502c1e23820967096c82a5de5df34))
- *(wasm)* Add signOrderWithBrowserWallet for EIP-1193 wallet signing (#29) ([`fd2352c`](https://github.com/deltartificial/cow-rs/commit/fd2352c921330069fb44b1ebf0747f4d4904f078))
- Add conformance test framework and maintainer CLI (#31) ([`8ba3f07`](https://github.com/deltartificial/cow-rs/commit/8ba3f0757fd9f29431f62bbd3ccf1d38561e1816))
- *(settlement)* Add SettlementEncoder, vault roles, and state readers (#32) ([`6271ab9`](https://github.com/deltartificial/cow-rs/commit/6271ab9b11350fe8cc4d2627a5fd8e43943353c0))
- Add injectable trait abstractions (OrderbookClient, CowSigner, RpcProvider) (#33) ([`b39d74e`](https://github.com/deltartificial/cow-rs/commit/b39d74ef898aabae5312ef293de36c548de7f6af))
- BrowserWallet SDK struct + orderbook/trading conformance fixtures (#34) ([`024b535`](https://github.com/deltartificial/cow-rs/commit/024b535f98aa08f83ca707eb7b8753893dc0eee6))
- Domain separator override, IpfsClient trait, ADRs (#35) ([`fb346ba`](https://github.com/deltartificial/cow-rs/commit/fb346ba2d1d5eadc03aff9ae730280d50fea4abf))
- *(browser-wallet)* Session management, events, chain switching, stateful mock (#36) ([`71503aa`](https://github.com/deltartificial/cow-rs/commit/71503aad069d52a648aeef140eee0e9bd646dda9))
- *(settlement)* Add TradeSimulator and order refund utilities (#37) ([`41c7ed4`](https://github.com/deltartificial/cow-rs/commit/41c7ed417e506bc91544578d8a11cee9f96d8cca))

### Miscellaneous

- *(codegen)* Pin upstream commit in orderbook spec provenance ([`62c3a13`](https://github.com/deltartificial/cow-rs/commit/62c3a13135f945b8f921e3694692e0904a1408da))
- *(ci)* Remove minimal-versions job and clean up version pins (#11) ([`bd6ca1a`](https://github.com/deltartificial/cow-rs/commit/bd6ca1ad97f4a2cdbf15263633d1c94750e6c537))
- *(ci)* Remove miri job — hangs and times out on this workspace (#12) ([`999a63b`](https://github.com/deltartificial/cow-rs/commit/999a63bc9cd0c34a42619b9a34cbe40c92a1380d))
- Move conformance/ into scripts/conformance/ (#39) ([`2169429`](https://github.com/deltartificial/cow-rs/commit/2169429d49c4f8067ab84999fad422c5aa1346a2))
- Bump workspace version to 0.1.0 for initial crates.io release ([`06f66fd`](https://github.com/deltartificial/cow-rs/commit/06f66fd4383b233b0c448beff55125e0916ef680))
- *(release)* Relocate specs into crate and add publish metadata ([`36c6e8f`](https://github.com/deltartificial/cow-rs/commit/36c6e8f8df497a1d6b44d79686aab374fba436c9))
- *(release)* 0.1.1 ([`b6c2d82`](https://github.com/deltartificial/cow-rs/commit/b6c2d825e46e415cc4eacd67f943a8ac6414e986))

### Refactor

- *(subgraph)* Replace lying schema tests with a real query linter ([`250e07f`](https://github.com/deltartificial/cow-rs/commit/250e07fa05d7b6c41867c33a5ac58ec249ee2032))

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

- *(readme)* Apply dprint table formatting ([`4a0ff3a`](https://github.com/deltartificial/cow-rs/commit/4a0ff3a5550e87c8ab7aaa464fa453487ab30ab8))


