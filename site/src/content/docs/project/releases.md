---
title: Development & releases
description: Build Steward, run checks, publish independent releases, and deploy the documentation site.
---

## Local checks

```sh
cargo fmt --check
cargo test --locked -j 1 -- --test-threads=1
cargo clippy --locked -j 1 --all-targets -- -D warnings
python3 -B -m unittest discover -s scripts -p 'test_*.py'
```

Rust checks run on Linux/macOS with a separate Rust 1.89 minimum-version check. Fixtures and HTTP/Unix mocks do not mutate production proxies or native services. Installer tests use local fake downloads and temporary destination directories.

## Documentation

The same repository contains an Astro Starlight project in `site/`:

```sh
npm ci --prefix site
npm run dev --prefix site
npm run build --prefix site
```

The local site uses `/steward/` as its base path. GitHub Pages deploys changes to the site and root installer on `main`. Its source must be **GitHub Actions** in repository Pages settings. Documentation search is built into the static site.

`site/src/pages/install.sh.ts` republishes the root channel selector directly, avoiding a separate installer copy. The example downloads likewise import their actual repository sources.

## Tagged releases

Update Cargo’s package version and lockfile, review scope/release notes, then publish a matching semantic tag:

```sh
git tag v0.1.0-alpha.1
git push origin v0.1.0-alpha.1
```

Use a new version for each release. The workflow requires the tag to match `v` plus the Cargo package version.

The release matrix builds and executes all four native targets:

- Linux x86_64 musl.
- Linux ARM64 musl.
- macOS Intel.
- macOS Apple Silicon.

Linux executables are checked for static linking. Each target ships as `steward_vVERSION_TARGET.tar.gz`, containing the CLI and README, with a SHA256 sidecar. A combined `SHA256SUMS` catalogs all archives and the release installer.

## Bedouin-generated installers

The publish job verifies and downloads the pinned Bedouin 0.23.1 generator, then runs:

```sh
bedouin install samishal1998/steward@v0.1.0-alpha.1 --generate --bin steward
```

`scripts/prepare_installer.py` validates Bedouin’s four resolved platform URLs and nonempty binary digests, then emits the release-pinned shell installer with strict verification and atomic placement. There is no second asset-guessing algorithm in shell.

Binaries/checksums must be publicly readable for generation. The workflow first publishes them as a non-latest prerelease, then attaches the verified installer, exercises it, and promotes only stable tags to the stable/latest channel. Alpha tags remain prereleases.

If a publish attempt fails, rerunning can complete the installer stage. Already public binary digests must match the downloaded artifacts; immutable binaries are not overwritten. An interrupted installer publication fails before placement rather than installing an unverified script.

The first Pages installer explicitly falls back to v0.1.0-alpha.1 only when no stable release exists. Maintain that documented fallback when intentionally changing the initial preview channel. Explicit `--version` selection remains available for any published tag.

## Release evidence boundary

Native executable/installer checks prove packaging and the current read-only slice on those targets. They do not certify future proxy activation, owned writes, systemd/launchd behavior, reboot/logout, or controller refresh. Those remain the separate acceptance gates in the product plan.
