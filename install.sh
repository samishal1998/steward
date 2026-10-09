#!/bin/sh
# A small channel selector. Release-specific installers contain pinned binary digests.
set -eu

main() {
  version=${STEWARD_VERSION:-latest}
  case "${1:-}" in --help|-h)
    printf '%s\n' 'Usage: sh install.sh [--version vX.Y.Z] [--dir /absolute/path]' \
      'Default: latest stable, or the documented read-only alpha when none exists.'
    return ;;
  esac
  # Validate options before downloading; forward only the install-directory option.
  dir=${STEWARD_BIN_DIR:-${BEDOUIN_BIN_DIR:-"${HOME:?HOME is not set}/.local/bin"}}
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --version|--dir)
        [ "$#" -ge 2 ] && [ -n "$2" ] || { printf 'Missing value: %s\n' "$1" >&2; exit 2; }
        if [ "$1" = --version ]; then version=$2; else dir=$2; fi
        shift 2 ;;
      *) printf 'Unknown option: %s\n' "$1" >&2; exit 2 ;;
    esac
  done
  case "$dir" in /*) ;; *) printf '%s\n' 'Install directory must be absolute.' >&2; exit 2 ;; esac
  for tool in curl mktemp awk; do command -v "$tool" >/dev/null 2>&1 || { printf 'Required command: %s\n' "$tool" >&2; exit 1; }; done
  repo=https://github.com/samishal1998/steward
  if [ "$version" = latest ]; then
    info=$(curl --silent --show-error --location --retry 3 --connect-timeout 15 --max-time 120 \
      --proto '=https' --proto-redir '=https' --output /dev/null --write-out '%{http_code}|%{url_effective}' "$repo/releases/latest")
    case "$info" in
      "200|$repo/releases/tag/"*)
        version=${info##*/}
        case "$version" in *-*) version=v0.1.0-alpha.1; printf '%s\n' 'No stable Steward release yet; installing the documented read-only alpha v0.1.0-alpha.1.' >&2 ;; esac ;;
      "200|$repo/releases"|"200|$repo/releases/"|"404|"*) version=v0.1.0-alpha.1; printf '%s\n' 'No stable Steward release yet; installing the read-only alpha v0.1.0-alpha.1.' >&2 ;;
      *) printf '%s\n' 'Could not resolve a Steward release. Choose --version or check GitHub Releases.' >&2; exit 1 ;;
    esac
  fi
  case "$version" in v*) ;; *) version=v$version ;; esac
  printf '%s\n' "$version" | awk '/^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$/ {valid=1} END {exit !valid}' || { printf '%s\n' 'Invalid release version.' >&2; exit 2; }
  if command -v sha256sum >/dev/null 2>&1; then checksum=sha256sum
  elif command -v shasum >/dev/null 2>&1; then checksum=shasum
  else printf '%s\n' 'Install sha256sum or shasum to verify the installer.' >&2; exit 1
  fi
  umask 077
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/steward-bootstrap.XXXXXX")
  trap 'rm -rf "$tmp"' 0
  trap 'exit 130' INT
  trap 'exit 143' TERM
  for file in install.sh SHA256SUMS; do
    curl --fail --silent --show-error --location --retry 3 --connect-timeout 15 --max-time 120 \
      --proto '=https' --proto-redir '=https' --output "$tmp/$file" "$repo/releases/download/$version/$file"
  done
  expected=$(awk '$2=="install.sh" {print $1; n++} END {if(n!=1) exit 1}' "$tmp/SHA256SUMS") || { printf '%s\n' 'Release installer checksum is missing or ambiguous.' >&2; exit 1; }
  [ "${#expected}" -eq 64 ] || { printf '%s\n' 'Invalid installer checksum.' >&2; exit 1; }
  case "$expected" in *[!0-9a-fA-F]*) printf '%s\n' 'Invalid installer checksum.' >&2; exit 1 ;; esac
  if [ "$checksum" = sha256sum ]; then got=$(sha256sum "$tmp/install.sh"); else got=$(shasum -a 256 "$tmp/install.sh"); fi
  [ "${got%% *}" = "$expected" ] || { printf '%s\n' 'Installer checksum mismatch; nothing was installed.' >&2; exit 1; }
  sh "$tmp/install.sh" --dir "$dir"
}

# The complete function must arrive before any download/install begins.
main "$@"
