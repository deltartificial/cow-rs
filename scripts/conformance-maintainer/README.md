# conformance-maintainer

CLI tool for managing the CoW conformance test framework. Validates the Rust SDK
against upstream TypeScript SDK test vectors.

## Build

```bash
cargo build --manifest-path scripts/conformance-maintainer/Cargo.toml
```

## Usage

### snapshot — Pin upstream commits

```bash
# Show current lock summary
conformance-maintainer snapshot

# Update cow-sdk and contracts commits from local checkouts
conformance-maintainer snapshot \
  --cow-sdk-root ~/src/cow-sdk \
  --contracts-root ~/src/contracts
```

### validate — Check fixture consistency

```bash
conformance-maintainer validate
```

Checks that all fixtures have `schema_version == 1`, that source ref commits
match the pinned commits in `source-lock.yaml`, and that every surface listed in
the lock has a corresponding fixture file.

Exit code 0 on success, 1 on errors.

### vendor-schemas — Copy app-data JSON schemas

```bash
conformance-maintainer vendor-schemas --cow-sdk-root ~/src/cow-sdk
```

Copies `packages/app-data/schemas/v*.json` from the cow-sdk checkout into
`specs/app-data/`, after verifying the checkout matches the pinned commit.
