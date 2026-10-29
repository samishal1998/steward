---
title: Development & releases
description: Build Steward, verify native packages, generate Bedouin-backed installers, and deploy Astro Starlight to GitHub Pages.
---

## Local checks

```sh
cargo fmt --check
cargo test --locked -j 1 -- --test-threads=1
cargo clippy --locked -j 1 --all-targets -- -D warnings
python3 -B -m unittest discover -s scripts -p 'test_*.py'
```

Source checks include Linux/macOS native acceptance and a separate Rust 1.89
minimum-version job. The live scripts use disposable proxies, uniquely named test
services and explicit cleanup. See the source evidence record for executed cases.

## Starlight and GitHub Pages

```sh
npm ci --prefix site
npm run dev --prefix site
npm run build --prefix site
```

Local base path: `/steward/`. `.github/workflows/pages.yml` builds with Node.js 24
and deploys changes on `main`. GitHub Pages must use **GitHub Actions** as its source.
The workflow takes the actual origin/base path from `actions/configure-pages`.

The site includes search, light/dark themes, Waymark branding, installation and
management guides, CLI reference, `llms.txt`/`llms-full.txt`, and real example/schema
downloads. The root installer, agent guide and examples are imported directly from
repository sources instead of maintained as separate copies.

## Publish a tag

1. Update `Cargo.toml` and `Cargo.lock` to the release version.
2. Align current docs, the documented installer fallback and installer fixtures.
3. Wait for **Rust checks** to pass on that exact commit.
4. Create a matching tag, such as `v0.2.0-alpha.2`.

```sh
git tag -a v0.2.0-alpha.2 -m "Steward v0.2.0-alpha.2"
git push origin v0.2.0-alpha.2
```

The tag-triggered release workflow requires `v` plus the Cargo version. Publishing
also checks for successful source acceptance on the exact tagged commit.

## Native release matrix

| Target | Runner |
| --- | --- |
| Linux x86_64 musl | Ubuntu 24.04 x86_64 |
| Linux ARM64 musl | Ubuntu 24.04 ARM64 |
| macOS Intel | macOS 15 Intel |
| macOS Apple Silicon | Native ARM64 macOS runner |

Builds use Rust 1.98.0, locked dependencies and one Cargo worker, with at most two
matrix jobs at once. Every target executes the built CLI, confirms version/command
contracts, and verifies its extracted archive. The real release executable also
runs route/TLS and PTY acceptance. Linux archives are checked for static linking.

Each `steward_vVERSION_TARGET.tar.gz` includes the CLI, README, agent guide, and
offline examples/fixtures. Each archive has a SHA256 sidecar; `SHA256SUMS` catalogs
the four archives and release installer. Package output refuses existing assets.

## Bedouin-backed installer generation

The publish job downloads **Bedouin standalone installer 0.23.1**, verifies its
upstream checksum/version, and runs:

```sh
bedouin-installer samishal1998/steward@v0.2.0-alpha.2 --generate --bin steward
```

`scripts/prepare_installer.py` validates the exact four platform URLs and digests
resolved by Bedouin, then emits Steward's pinned installer with checksum/version
verification and atomic placement. The standalone generator requires no machine
provisioning config. Users installing Steward do not need Bedouin or Python.

The root `install.sh` is a small channel selector. It resolves stable/pinned
versions, verifies the release installer against `SHA256SUMS`, and executes that
verified script. The release script verifies the archive and executable version
before placing the binary. The normal installation command is:

```sh
curl -fsSL https://samishal1998.github.io/steward/install.sh | sh
```

## Publication order and retry

Bedouin must resolve publicly readable release assets. The workflow initially
publishes immutable archives/digests as a non-latest prerelease, attaches the
verified installer/catalog, then exercises installation from the public release.
Only a successful stable tag becomes latest; alpha tags stay prereleases.

For the first publication of a new fallback, finish the tag release before
advancing the public Pages source. This keeps the public selector pointed at a
downloadable release throughout deployment.

If all four builds succeeded but publishing failed, reuse their original artifacts:

```sh
gh workflow run release.yml --repo samishal1998/steward --ref main \
  -f release_tag=v0.2.0-alpha.2 -f build_run_id=BUILD_RUN_ID
```

Recovery verifies the tag commit, original workflow/run identity, four successful
native jobs and already published binary digests. It does not move the tag or
replace public archives. If artifacts have expired or code must change, publish
a new version. An incomplete/mismatched installer publication fails before placement.
