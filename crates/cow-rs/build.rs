//! Build script for the `cow-rs` crate.
//!
//! Generates the `CoW` Protocol orderbook REST API client from the `OpenAPI`
//! specification at `specs/orderbook-api.yml` using [`progenitor`].
//!
//! The generated code is written to `$OUT_DIR/orderbook_generated.rs` and
//! included via `include!` in `src/order_book/generated.rs`.

// Build scripts legitimately need stdio (`cargo::rerun-if-changed` is
// communicated to cargo via stdout) and panicking on setup failure is
// the standard build-script error contract. The workspace lint profile
// targets library code, not build-time tooling, so we opt out here.
#![allow(
    clippy::disallowed_macros,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "build-script: stdio + panic are the cargo contract"
)]

use std::{env, fs, path::Path};

fn main() {
    // Rebuild when specs change (even those used only in tests).
    println!("cargo::rerun-if-changed=../../specs/subgraph.graphql");
    println!("cargo::rerun-if-changed=../../specs/app-data-schema.json");

    generate_orderbook_client();
}

fn generate_orderbook_client() {
    let spec_path = Path::new("../../specs/orderbook-api.yml");
    println!("cargo::rerun-if-changed={}", spec_path.display());
    println!("cargo::rerun-if-changed=build.rs");

    // Parse YAML → JSON value so we can patch before deserializing to OpenAPI.
    let spec_yaml = fs::read_to_string(spec_path)
        .unwrap_or_else(|e| panic!("failed to read OpenAPI spec at {}: {e}", spec_path.display()));
    let mut spec_value: serde_json::Value = serde_yaml::from_str(&spec_yaml).unwrap_or_else(|e| {
        panic!("failed to parse OpenAPI spec: {e}");
    });

    // Patch the spec for progenitor 0.13 compatibility.
    patch_spec_for_progenitor(&mut spec_value);

    let spec: openapiv3::OpenAPI = serde_json::from_value(spec_value).unwrap_or_else(|e| {
        panic!("failed to deserialize OpenAPI spec: {e}");
    });

    let mut generator = progenitor_impl::Generator::new(
        progenitor_impl::GenerationSettings::default()
            .with_interface(progenitor_impl::InterfaceStyle::Positional)
            .with_tag(progenitor_impl::TagStyle::Merged)
            .with_derive("PartialEq"),
    );

    let tokens = generator
        .generate_tokens(&spec)
        .unwrap_or_else(|e| panic!("OpenAPI code generation failed: {e}"));

    let ast: syn::File =
        syn::parse2(tokens).unwrap_or_else(|e| panic!("failed to parse generated code: {e}"));
    let code = prettyplease::unparse(&ast);

    let out_dir = env::var("OUT_DIR").unwrap_or_else(|e| panic!("OUT_DIR not set: {e}"));
    let out_path = Path::new(&out_dir).join("orderbook_generated.rs");
    fs::write(&out_path, code).unwrap_or_else(|e| {
        panic!("failed to write generated code to {}: {e}", out_path.display())
    });
}

/// Patch the `OpenAPI` spec to work around progenitor 0.13 limitations.
///
/// Progenitor asserts `response_types.len() <= 1` — all responses in a
/// response-class (success or error) must resolve to the same
/// `OperationResponseKind`. The upstream `CoW` spec violates this in two ways:
///
/// 1. **Duplicate success codes**: `PUT /api/v1/app_data` defines both 200 and 201 with identical
///    schemas. Fix: remove duplicate 2xx codes.
///
/// 2. **Mixed error types**: Some endpoints have 400 with `application/json` content alongside
///    403/404/422/etc with no content. This produces `{Type(..), None}` in the error type set. Fix:
///    strip JSON content from error responses so all resolve to `None`.
fn patch_spec_for_progenitor(spec: &mut serde_json::Value) {
    let Some(paths) = spec.get_mut("paths").and_then(|p| p.as_object_mut()) else {
        return;
    };

    for (path, methods) in paths.iter_mut() {
        let Some(methods_obj) = methods.as_object_mut() else {
            continue;
        };
        for (method, details) in methods_obj.iter_mut() {
            let Some(responses) = details.get_mut("responses").and_then(|r| r.as_object_mut())
            else {
                continue;
            };

            // Fix 1: remove duplicate 2xx success responses (keep lowest code).
            let success_codes: Vec<String> = responses
                .keys()
                .filter(|code| {
                    code.parse::<u16>().is_ok_and(|c| (200..300).contains(&c)) &&
                        responses[*code]
                            .get("content")
                            .and_then(|c| c.as_object())
                            .is_some_and(|c| !c.is_empty())
                })
                .cloned()
                .collect();

            if success_codes.len() > 1 {
                eprintln!(
                    "build.rs: patch {method} {path}: removing {} duplicate 2xx responses",
                    success_codes.len() - 1
                );
                for code in &success_codes[1..] {
                    responses.remove(code);
                }
            }

            // Fix 2: normalize error responses — if any 4xx/5xx has JSON
            // content while others don't, strip the content from the typed
            // ones so progenitor sees a uniform `None` error type.
            let error_codes: Vec<String> = responses
                .keys()
                .filter(|code| code.parse::<u16>().is_ok_and(|c| (400..600).contains(&c)))
                .cloned()
                .collect();

            let has_typed_error = error_codes.iter().any(|code| {
                responses[code]
                    .get("content")
                    .and_then(|c| c.as_object())
                    .is_some_and(|c| !c.is_empty())
            });
            let has_untyped_error = error_codes.iter().any(|code| {
                responses[code]
                    .get("content")
                    .and_then(|c| c.as_object())
                    .map_or(true, |c| c.is_empty())
            });

            if has_typed_error && has_untyped_error {
                eprintln!(
                    "build.rs: patch {method} {path}: stripping typed error content \
                     (mixed typed/untyped errors)"
                );
                for code in &error_codes {
                    if let Some(resp) = responses.get_mut(code) &&
                        let Some(obj) = resp.as_object_mut()
                    {
                        obj.remove("content");
                    }
                }
            }
        }
    }
}
