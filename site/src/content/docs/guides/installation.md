---
title: Installation
description: Install Steward's direct-route alpha on Linux or macOS without building from source.
---

Steward **alpha.2** adds shell completions and direct, recoverable local HTTP route commands to native Caddy/Traefik inventory. Proxy installation, broader controller/TLS modes, native services, composition and TUI retain their roadmap gates.

## Linux and macOS

```sh
curl -fsSL https://samishal1998.github.io/steward/install.sh | sh
```

Then:

```sh
~/.local/bin/steward --version
~/.local/bin/steward --help
```

Supported downloads:

| System | Architecture | Build |
| --- | --- | --- |
| Linux | x86_64 / AMD64 | Static musl |
| Linux | ARM64 / aarch64 | Static musl |
| macOS | Intel x86_64 | Native macOS executable |
| macOS | Apple Silicon ARM64 | Native macOS executable |

The installer defaults to GitHub's latest stable release, or an explicitly documented alpha when none exists. Pin **v0.1.0-alpha.2** below for the direct commands; newer prereleases do not silently become stable updates.

It verifies the release installer’s checksum, then the binary archive’s pinned checksum and the executable’s product/version. Installation is staged and atomically placed in `~/.local/bin/steward`. It does not require sudo, change shell configuration, start services, or enable a proxy API.

The machine needs `curl`, `tar`, standard Unix tools, and either `sha256sum` or `shasum`. Node.js, Python, Rust, and Bedouin are not runtime installer dependencies. HTTPS inspection uses the operating system’s CA roots; minimal Linux images need their usual CA-certificate bundle for trusted HTTPS.

## Pin a release or change the directory

```sh
curl -fsSL https://samishal1998.github.io/steward/install.sh |
  sh -s -- --version v0.1.0-alpha.2 --dir "$HOME/bin"
```

`STEWARD_VERSION` and `STEWARD_BIN_DIR` are also accepted. `BEDOUIN_BIN_DIR` is supported for compatibility with Bedouin-generated installers. Custom installation directories must be absolute and writable by your user.

The release’s own `install.sh` is pinned to that release and supports `--dir` only. The Pages installer is the channel selector that additionally supports `--version`.

## Inspect before executing

```sh
curl -fsSL https://samishal1998.github.io/steward/install.sh -o install-steward.sh
less install-steward.sh
sh install-steward.sh
```

Checksum checks provide download integrity under the GitHub/HTTPS trust boundary; they are not an independent publisher signature. Download/checksum/version failures leave the previous installation intact.

## Use Bedouin

If Bedouin is already installed:

```sh
bedouin-installer samishal1998/steward@v0.1.0-alpha.2
```

Maintainers use the standalone Bedouin generator's `--generate` mode to resolve platform URLs/digests without a provisioning config. The shell installer itself does not require Bedouin.

## Manual downloads

[GitHub Releases](https://github.com/samishal1998/steward/releases) includes one archive per target, SHA256 sidecars, a combined `SHA256SUMS`, and the release-pinned installer. Extract `steward`, verify the archive against its digest, and place the executable in your desired bin directory.

Windows is a later target; this release does not advertise a Windows build.

## Build from source

```sh
git clone https://github.com/samishal1998/steward.git
cd steward
cargo build --locked -j 1
./target/debug/steward --help
```

Requires Rust 1.89 or newer. Source builds and release downloads use the same standalone CLI. Next, [get started with direct commands](../quick-start/) and [enable shell completion](../completion/).
