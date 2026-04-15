---
name: release-publish
description: Publish kham crates to crates.io, npm, and PyPI. Use when preparing a release, bumping versions, publishing packages, or setting up CI/CD for releases.
---

# Release & Publish Guide

## Publish Order (dependency chain)

Crates MUST be published in this exact order:

```
1. kham-core      → crates.io
2. kham-capi      → crates.io (depends on kham-core)
3. kham-cli       → crates.io (depends on kham-core)
4. kham-wasm      → npm (depends on kham-core)
5. kham-python    → PyPI (depends on kham-core)
```

Never publish a downstream crate before its dependency is live on crates.io.

## Version Bumping

All crates share the same version. Use workspace inheritance:

```toml
# root Cargo.toml
[workspace.package]
version = "0.1.0"

# each crate's Cargo.toml
[package]
version.workspace = true
```

Bump in one place. Use semver:
- PATCH: bug fix, dictionary update
- MINOR: new API, new feature, new binding
- MAJOR: breaking API change in kham-core

## Pre-publish Checklist

```bash
# 1. All tests pass
cargo test --workspace

# 2. No warnings
cargo clippy --workspace -- -D warnings

# 3. Docs build
cargo doc --workspace --no-deps

# 4. Dry run publish
cargo publish -p kham-core --dry-run

# 5. CHANGELOG.md updated

# 6. Git tag
git tag -a v0.1.0 -m "Release 0.1.0"
git push origin v0.1.0
```

## crates.io Publish

```bash
cargo publish -p kham-core
# Wait ~1 min for crates.io index update
cargo publish -p kham-capi
cargo publish -p kham-cli
```

## npm Publish (kham-wasm)

```bash
cd kham-wasm
wasm-pack build --target web --release
cd pkg
# Verify package.json has correct name: "kham"
npm publish
```

## PyPI Publish (kham-python)

```bash
cd kham-python
maturin build --release
maturin publish
```

## CI Release (GitHub Actions)

Tag push `v*` triggers: test → build all targets → publish cascade.
Binary artifacts: build CLI for linux-x64, macos-arm64, windows-x64.
