# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Features

- Add 5 fuzz targets covering core SDK parsers (#13) ([`7ee11fb`](https://github.com/deltartificial/cow-rs/commit/7ee11fb9840047ce87ccb774b380e46a2b699ff1))
- Complete fuzz coverage with 8 additional fuzz targets (#14) ([`6b77300`](https://github.com/deltartificial/cow-rs/commit/6b77300d5e6f4a48b323fea330895cda1df8c6b3))

### Bug Fixes

- Gate tokio usage on `native` feature and pin alloy-consensus minimum (#3) ([`deab597`](https://github.com/deltartificial/cow-rs/commit/deab597ecabfebdcea8c751697950f72195dfc8f))
- Pin tracing ≥0.1.40 for minimal-versions compatibility (#4) ([`70e6fd6`](https://github.com/deltartificial/cow-rs/commit/70e6fd62ce9f2aebab934095209f573f43482ef3))
- Skip hashbrown 0.16 duplicate in cargo-deny (#5) ([`389ebb8`](https://github.com/deltartificial/cow-rs/commit/389ebb894ac2a05ba2ffadf24841be7d92a07340))
- Pin transitive deps for minimal-versions and fix dprint format (#6) ([`40736d7`](https://github.com/deltartificial/cow-rs/commit/40736d7269bb36c5cd7c78877381fed2cc6fedee))
- Sort workspace and crate deps alphabetically (#7) ([`7b7eba8`](https://github.com/deltartificial/cow-rs/commit/7b7eba8ac1b91834f61ba6924ca11da88d79a63d))
- Pin ahash, ref-cast, alloy-rpc-types-eth for minimal-versions (#8) ([`57e7d35`](https://github.com/deltartificial/cow-rs/commit/57e7d358d16794f06105a8203a7a8756ee460153))
- Pin alloy-signer ≥1.5 for minimal-versions (#9) ([`f7f6d83`](https://github.com/deltartificial/cow-rs/commit/f7f6d83a766660e1d70d51dbc85245a3bd87c07f))
- Bump alloy-signer pin to ≥1.8.3 for minimal-versions (#10) ([`2b70317`](https://github.com/deltartificial/cow-rs/commit/2b70317e7d388b5c06040219acf986a76028e8dd))
- Resolve clippy and fmt errors in test code (#23) ([`73943dd`](https://github.com/deltartificial/cow-rs/commit/73943ddf68cf9d217318da1510b68f9c496a06f1))
- Use Arc::clone instead of .clone() on ref-counted pointer (#27) ([`f0ab089`](https://github.com/deltartificial/cow-rs/commit/f0ab089))

### Tests

- Add unit tests for types, contracts, params, calldata (42% → 48%) (#15) ([`790bb11`](https://github.com/deltartificial/cow-rs/commit/790bb1106a1949196d8c52e66923e37f04e6cc2f))
- Comprehensive unit tests across SDK (42% → 82% coverage) (#16) ([`7ac98a0`](https://github.com/deltartificial/cow-rs/commit/7ac98a07331f5b51840c2774a0e97f2453716fa8))
- Wiremock API tests + sync coverage push (82% → 85%) (#17) ([`e79eb0f`](https://github.com/deltartificial/cow-rs/commit/e79eb0feedc52499d4d3bda60ce5af05e969f2ef))
- Bridging + IPFS wiremock tests (85% → 90% coverage) (#18) ([`582430e`](https://github.com/deltartificial/cow-rs/commit/582430ed21fedbdec046acb37f5ec2a0a8764820))
- Push coverage to 92% with expanded wiremock tests (#19) ([`4af871b`](https://github.com/deltartificial/cow-rs/commit/4af871bc0351399b02a01fb56ef6d12e8bdd3d5c))
- Push coverage to 96% — trading/sdk, order_book, bridging, onchain (#26) ([`c90351d`](https://github.com/deltartificial/cow-rs/commit/c90351d))

### CI

- Remove minimal-versions job and clean up version pins (#11) ([`bd6ca1a`](https://github.com/deltartificial/cow-rs/commit/bd6ca1ad97f4a2cdbf15263633d1c94750e6c537))
- Remove miri job — hangs and times out on this workspace (#12) ([`999a63b`](https://github.com/deltartificial/cow-rs/commit/999a63bc9cd0c34a42619b9a34cbe40c92a1380d))
- Add shields.io badges and Codecov coverage workflow (#20) ([`6643c31`](https://github.com/deltartificial/cow-rs/commit/6643c31b7e20addc54b7f211f5e6f4e5f5a00981))
- Add CODECOV_TOKEN to coverage upload (#24) ([`764197d`](https://github.com/deltartificial/cow-rs/commit/764197d12d8261477efc35d0e0fff675b3b45ab8))

### Docs

- Remove MSRV badge (#21) ([`94e6119`](https://github.com/deltartificial/cow-rs/commit/94e6119bf8285cf92bd740c5c282aeac0e9586f4))
- Add blank line between badges and description (#22) ([`524eba8`](https://github.com/deltartificial/cow-rs/commit/524eba837667779aad5a4f36431b9e5f9dbecda5))

## [1.0.0] - 2026-03-03

### Features

- Add CoW Protocol Rust SDK (#2) ([`0d1d3bd`](https://github.com/deltartificial/cow-rs/commit/0d1d3bde83e524c6032538411fb90cbc61fceed3))
