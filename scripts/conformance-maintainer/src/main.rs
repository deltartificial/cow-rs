use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, bail, ensure};
use chrono::Utc;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "conformance-maintainer", about = "Manage CoW conformance test fixtures")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Update source-lock.yaml with current upstream commits.
    Snapshot {
        /// Path to a local cow-sdk checkout.
        #[arg(long)]
        cow_sdk_root: Option<PathBuf>,
        /// Path to a local cow-contracts checkout.
        #[arg(long)]
        contracts_root: Option<PathBuf>,
    },
    /// Check consistency between source-lock and fixtures.
    Validate,
    /// Copy app-data JSON schemas from a cow-sdk checkout.
    VendorSchemas {
        /// Path to a local cow-sdk checkout.
        #[arg(long)]
        cow_sdk_root: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// source-lock.yaml model
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
struct SourceLock {
    repositories: BTreeMap<String, RepoEntry>,
    surfaces: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RepoEntry {
    url: String,
    commit: String,
    description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reference_only: Option<bool>,
    pinned_at: String,
}

// ---------------------------------------------------------------------------
// fixture model (just enough to validate)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Fixture {
    schema_version: u32,
    surface: String,
    #[allow(dead_code)]
    generated_at_utc: String,
    source_refs: Vec<SourceRef>,
}

#[derive(Debug, Deserialize)]
struct SourceRef {
    repo: String,
    commit: String,
    #[allow(dead_code)]
    path: String,
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Walk up from the tool's own directory to find the workspace root
/// (the directory that contains `scripts/conformance/source-lock.yaml`).
fn find_workspace_root() -> anyhow::Result<PathBuf> {
    // Start from CARGO_MANIFEST_DIR if available, else current dir.
    let start = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("cannot determine cwd"));
    let mut dir = start.as_path();
    loop {
        if dir.join("scripts/conformance/source-lock.yaml").exists() {
            return Ok(dir.to_path_buf());
        }
        dir = dir
            .parent()
            .context("reached filesystem root without finding workspace")?;
    }
}

fn load_source_lock(ws: &Path) -> anyhow::Result<SourceLock> {
    let path = ws.join("scripts/conformance/source-lock.yaml");
    let text = fs::read_to_string(&path).context("reading source-lock.yaml")?;
    serde_yaml::from_str(&text).context("parsing source-lock.yaml")
}

fn save_source_lock(ws: &Path, lock: &SourceLock) -> anyhow::Result<()> {
    let path = ws.join("scripts/conformance/source-lock.yaml");
    // Preserve the header comment.
    let header = "\
# Pinned upstream source contract for parity fixtures.
# Each fixture case proves that our Rust SDK produces identical output
# to the TypeScript SDK at the pinned commit.
#
# Workflow:
#   1. Pin upstream commits here
#   2. Extract test vectors from TS SDK at those commits
#   3. Commit fixtures as reviewable JSON
#   4. Rust tests load fixtures and assert output == expected
#
# To update: change commits, re-extract fixtures, run `cargo nextest run`
";
    let yaml = serde_yaml::to_string(lock).context("serializing source-lock")?;
    fs::write(&path, format!("{header}\n{yaml}")).context("writing source-lock.yaml")?;
    Ok(())
}

fn git_head(repo_path: &Path) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .context("running git rev-parse HEAD")?;
    ensure!(output.status.success(), "git rev-parse failed in {}", repo_path.display());
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn load_fixture(path: &Path) -> anyhow::Result<Fixture> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading fixture {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("parsing fixture {}", path.display()))
}

// ---------------------------------------------------------------------------
// subcommands
// ---------------------------------------------------------------------------

fn cmd_snapshot(
    cow_sdk_root: Option<PathBuf>,
    contracts_root: Option<PathBuf>,
) -> anyhow::Result<()> {
    let ws = find_workspace_root()?;
    let mut lock = load_source_lock(&ws)?;

    let today = Utc::now().format("%Y-%m-%d").to_string();

    let roots: BTreeMap<&str, Option<&PathBuf>> = BTreeMap::from([
        ("cow-sdk", cow_sdk_root.as_ref()),
        ("cow-contracts", contracts_root.as_ref()),
    ]);

    let mut updated = false;

    for (key, root) in &roots {
        if let Some(path) = root {
            let commit = git_head(path)?;
            if let Some(entry) = lock.repositories.get_mut(*key) {
                println!("[snapshot] {key}: {} -> {commit}", entry.commit);
                entry.commit = commit;
                entry.pinned_at = today.clone();
                updated = true;
            } else {
                println!("[snapshot] WARNING: no repository entry for {key}");
            }
        }
    }

    if updated {
        save_source_lock(&ws, &lock)?;
        println!("[snapshot] source-lock.yaml updated");
    } else {
        println!("[snapshot] No roots provided — showing current lock summary:");
        for (name, entry) in &lock.repositories {
            let ref_only = entry.reference_only.unwrap_or(false);
            let tag = if ref_only { " (reference-only)" } else { "" };
            println!("  {name}: {}{tag}", &entry.commit[..12]);
        }
        println!("  surfaces: {:?}", lock.surfaces);
    }

    Ok(())
}

fn cmd_validate() -> anyhow::Result<()> {
    let ws = find_workspace_root()?;
    let lock = load_source_lock(&ws)?;

    let fixtures_dir = ws.join("scripts/conformance/fixtures");
    let mut errors: u32 = 0;
    let mut warnings: u32 = 0;

    // Collect commit expectations from source-lock (skip reference-only repos).
    let expected_commits: BTreeMap<&str, &str> = lock
        .repositories
        .iter()
        .filter(|(_, e)| !e.reference_only.unwrap_or(false))
        .map(|(k, e)| (k.as_str(), e.commit.as_str()))
        .collect();

    // Load all fixtures.
    let mut fixture_surfaces: Vec<String> = Vec::new();

    for entry in fs::read_dir(&fixtures_dir).context("reading fixtures dir")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let fixture = load_fixture(&path)?;
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            fixture_surfaces.push(fixture.surface.clone());

            // Check schema_version.
            if fixture.schema_version != 1 {
                println!("ERROR  {name}: schema_version = {} (expected 1)", fixture.schema_version);
                errors += 1;
            } else {
                println!("OK     {name}: schema_version = 1");
            }

            // Check source_refs commits.
            for sr in &fixture.source_refs {
                if let Some(&expected) = expected_commits.get(sr.repo.as_str()) {
                    if sr.commit != expected {
                        println!(
                            "ERROR  {name}: source_ref {repo} commit mismatch: fixture={short_f} lock={short_l}",
                            repo = sr.repo,
                            short_f = &sr.commit[..12],
                            short_l = &expected[..12],
                        );
                        errors += 1;
                    } else {
                        println!("OK     {name}: source_ref {} commit matches", sr.repo);
                    }
                }
            }
        }
    }

    // Check that every surface in source-lock has a fixture.
    for surface in &lock.surfaces {
        if fixture_surfaces.contains(surface) {
            println!("OK     surface '{surface}' has a fixture file");
        } else {
            let fixture_path = fixtures_dir.join(format!("{surface}.json"));
            if fixture_path.exists() {
                println!("WARN   surface '{surface}' file exists but surface field doesn't match");
                warnings += 1;
            } else {
                println!("WARN   surface '{surface}' listed in source-lock but no fixture yet");
                warnings += 1;
            }
        }
    }

    println!();
    println!("Result: {errors} error(s), {warnings} warning(s)");

    if errors > 0 {
        bail!("validation failed with {errors} error(s)");
    }
    Ok(())
}

fn cmd_vendor_schemas(cow_sdk_root: &Path) -> anyhow::Result<()> {
    let ws = find_workspace_root()?;
    let lock = load_source_lock(&ws)?;

    let expected_commit = lock
        .repositories
        .get("cow-sdk")
        .context("no cow-sdk entry in source-lock")?
        .commit
        .clone();

    // Warn if checkout is at a different commit.
    let actual = git_head(cow_sdk_root)?;
    if actual != expected_commit {
        println!(
            "WARNING: cow-sdk checkout is at {}, pinned commit is {}",
            &actual[..12],
            &expected_commit[..12],
        );
    }

    let src_dir = cow_sdk_root.join("packages/app-data/schemas");
    ensure!(src_dir.is_dir(), "schemas dir not found at {}", src_dir.display());

    let dest_dir = ws.join("specs/app-data");
    fs::create_dir_all(&dest_dir)?;

    let mut count = 0u32;
    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('v') && name_str.ends_with(".json") {
            let dest = dest_dir.join(&name);
            fs::copy(entry.path(), &dest)?;
            println!("copied {name_str} -> {}", dest.display());
            count += 1;
        }
    }

    println!("Vendored {count} schema file(s)");
    Ok(())
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Snapshot {
            cow_sdk_root,
            contracts_root,
        } => cmd_snapshot(cow_sdk_root, contracts_root),
        Cmd::Validate => cmd_validate(),
        Cmd::VendorSchemas { cow_sdk_root } => cmd_vendor_schemas(&cow_sdk_root),
    }
}
