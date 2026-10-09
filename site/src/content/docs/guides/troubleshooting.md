---
title: Troubleshooting
description: Resolve download, path, proxy access, partial inventory, and alpha capability issues.
---

## `steward: command not found`

Use the full path:

```sh
~/.local/bin/steward --version
```

Or add `~/.local/bin` to your shell’s PATH. The installer does not edit shell files.

## No stable release is available

The project is in a read-only alpha. The Pages installer explicitly falls back to v0.1.0-alpha.1 when GitHub has no stable release. You can also select that version directly with `--version`, or use `bedouin install samishal1998/steward@v0.1.0-alpha.1 --yes`.

If a release is still being published and its installer is not available yet, installation fails before placement. Check that the [Release workflow](https://github.com/samishal1998/steward/actions/workflows/release.yml) has completed before retrying.

## Checksum or version mismatch

Do not bypass the check. Retry the intended release from its documented URL and confirm its assets are complete. A failed download/checksum/version check leaves an existing installation intact. The installer rejects destination symlinks/directories rather than replacing an unexpected target.

## Unsupported architecture

The release targets Linux/macOS x86_64 and ARM64. 32-bit ARM and Windows are not supported downloads in this alpha. Cross-compilation alone does not certify the future native-service capabilities.

## Proxy API cannot be reached

Verify the configured API base URL from the machine or container running Steward. `localhost` is that process’s context. A shared network does not make another container’s loopback-only admin listener reachable.

APIs must already be enabled and authorized. Steward does not open ports, enable dashboards, restart proxies, mount sockets, or change permissions during inventory.

## Authentication, redirects, or TLS errors

Configure an absolute readable authorization-file path with a complete `Basic ...` or `Bearer ...` value. Use the final API URL directly; redirects are rejected. HTTPS verification uses system trust, and the alpha does not have a switch to disable it or configure mTLS/private CA bundles.

## Inventory is partial

Inspect the diagnostics. Offline snapshots are always runtime-unconfirmed. Unknown matchers/modules, native error states, missing dependencies, or one failed binding can produce partial results. This is not an empty inventory or a claim that external routes are safe to overwrite.

## Traefik returns more pages

Steward requests `per_page=10001` and accepts at most 10,000 records per endpoint. Access proxies must preserve that query and `X-Next-Page`. A truncated response is rejected, not presented as a complete scan.

## A plan is not applicable

That is expected in the alpha. `applicable: false` means the mutation foundation and/or evidence prerequisites are not met. `--yes` does not enable writes. See [route planning](../planning/).

## Report an issue

[Open an issue](https://github.com/samishal1998/steward/issues) with your CLI version, OS/architecture, redacted binding shape, diagnostic codes, and reproduction steps. Use synthetic or scrubbed native responses. Keep tokens, authorization files, certificate material, and private process environments out of reports.
