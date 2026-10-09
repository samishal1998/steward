"""Validate Bedouin's resolved table and emit Steward's release-pinned installer."""
import argparse
import re
import sys

TARGETS = {
    "Linux-x86_64": "x86_64-unknown-linux-musl",
    "Linux-aarch64|Linux-arm64": "aarch64-unknown-linux-musl",
    "Darwin-x86_64": "x86_64-apple-darwin",
    "Darwin-arm64": "aarch64-apple-darwin",
}


def prepare(raw, version):
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("invalid semantic release tag")
    if not raw.startswith(f"#!/bin/sh\n# samishal1998/steward {version}\n"):
        raise ValueError("generator output is for another product/version")
    rows = re.findall(r"^  ([A-Za-z0-9_|-]+)\)\n    url='([^']+)'\n    file='([^']+)'\n    sum='([0-9a-f]{64})'\n    ;;", raw, re.M)
    if len(rows) != 4 or {r[0] for r in rows} != set(TARGETS):
        raise ValueError("generator must provide four unambiguous platforms with SHA256 digests")
    table = []
    for uname, url, file, digest in rows:
        expected = f"steward_{version}_{TARGETS[uname]}.tar.gz"
        if file != expected or url != f"https://github.com/samishal1998/steward/releases/download/{version}/{expected}":
            raise ValueError("unexpected release asset URL/name")
        table.append(f"    {uname}) url='{url}'; file='{file}'; sum='{digest}' ;;")
    return f'''#!/bin/sh
# samishal1998/steward {version}
# Platform URLs and digests resolved by Bedouin 0.23.1; placement validated by Steward.
set -eu

main() {{
  dir=${{STEWARD_BIN_DIR:-${{BEDOUIN_BIN_DIR:-"${{HOME:?HOME is not set}}/.local/bin"}}}}
  case "${{1:-}}" in --help|-h) printf '%s\\n' 'Steward {version}: sh install.sh [--dir /absolute/path]'; return ;; esac
  if [ "$#" -gt 0 ]; then
    [ "$#" -eq 2 ] && [ "$1" = --dir ] && [ -n "$2" ] || {{ printf '%s\\n' 'Use --dir /absolute/path; this installer is release-pinned.' >&2; exit 2; }}
    dir=$2
  fi
  case "$dir" in /*) ;; *) printf '%s\\n' 'Install directory must be absolute.' >&2; exit 2 ;; esac
  [ ! -d "$dir/steward" ] && [ ! -L "$dir/steward" ] || {{ printf '%s\\n' 'Destination steward is a directory or symlink.' >&2; exit 1; }}
  for tool in curl tar uname mktemp chmod mkdir cp mv; do command -v "$tool" >/dev/null 2>&1 || {{ printf 'Required command: %s\\n' "$tool" >&2; exit 1; }}; done
  if command -v sha256sum >/dev/null 2>&1; then checksum=sha256sum
  elif command -v shasum >/dev/null 2>&1; then checksum=shasum
  else printf '%s\\n' 'Install sha256sum or shasum to verify the download.' >&2; exit 1
  fi
  case "$(uname -s)-$(uname -m)" in
{chr(10).join(table)}
    *) printf '%s\\n' 'Supported systems: Linux/macOS x86_64 and ARM64.' >&2; exit 1 ;;
  esac
  umask 077
  tmp=$(mktemp -d "${{TMPDIR:-/tmp}}/steward-install.XXXXXX")
  staged=
  trap 'rm -rf "$tmp"; if [ -n "$staged" ]; then rm -f "$staged"; fi' 0
  trap 'exit 130' INT
  trap 'exit 143' TERM
  printf 'Downloading Steward {version}…\\n'
  curl --fail --silent --show-error --location --retry 3 --connect-timeout 15 --max-time 300 \\
    --proto '=https' --proto-redir '=https' --output "$tmp/$file" "$url"
  if [ "$checksum" = sha256sum ]; then got=$(sha256sum "$tmp/$file"); else got=$(shasum -a 256 "$tmp/$file"); fi
  [ "${{got%% *}}" = "$sum" ] || {{ printf '%s\\n' 'Checksum mismatch; the existing installation was not changed.' >&2; exit 1; }}
  tar -xzf "$tmp/$file" -C "$tmp" steward
  [ -f "$tmp/steward" ] && [ ! -L "$tmp/steward" ] || {{ printf '%s\\n' 'Release has no regular steward executable.' >&2; exit 1; }}
  chmod 755 "$tmp/steward"
  reported=$("$tmp/steward" --version)
  [ "$reported" = 'steward {version[1:]}' ] || {{ printf '%s\\n' 'Downloaded executable has an unexpected product/version.' >&2; exit 1; }}
  mkdir -p "$dir"
  staged=$(mktemp "$dir/.steward.XXXXXX")
  cp "$tmp/steward" "$staged"
  chmod 755 "$staged"
  mv -f "$staged" "$dir/steward"
  staged=
  printf '\\nInstalled %s to %s/steward\\n' "$reported" "$dir"
  case ":${{PATH:-}}:" in *":$dir:"*) ;; *) printf 'Add %s to PATH or use the full executable path.\\n' "$dir" ;; esac
  printf '%s\\n' 'Run steward --help for available commands. No proxy configuration or services were changed.'
}}

main "$@"
'''


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    raw = sys.stdin.read(128 * 1024 + 1)
    try:
        if len(raw) > 128 * 1024:
            raise ValueError("generator output exceeds 128 KiB")
        sys.stdout.write(prepare(raw, args.version))
    except ValueError as error:
        parser.exit(1, str(error) + "\n")
