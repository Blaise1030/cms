#!/bin/sh
# Install the orso CLI from GitHub Releases.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Blaise1030/cms/main/scripts/install.sh | sh
# Optional env:
#   ORSO_VERSION=0.1.2   pin a version (without leading v); default = latest release
#   ORSO_REPO=Blaise1030/cms
#   ORSO_INSTALL_DIR=~/.local/bin
set -eu

REPO="${ORSO_REPO:-Blaise1030/cms}"
INSTALL_DIR="${ORSO_INSTALL_DIR:-${HOME}/.local/bin}"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "orso install: need '$1' on PATH" >&2
    exit 1
  }
}

need curl
need tar
need mktemp

uname_s=$(uname -s)
uname_m=$(uname -m)
case "$uname_s" in
  Darwin)
    case "$uname_m" in
      arm64 | aarch64) TARGET=aarch64-apple-darwin ;;
      x86_64) TARGET=x86_64-apple-darwin ;;
      *)
        echo "orso install: unsupported macOS arch: $uname_m" >&2
        exit 1
        ;;
    esac
    ;;
  Linux)
    case "$uname_m" in
      aarch64 | arm64) TARGET=aarch64-unknown-linux-gnu ;;
      x86_64) TARGET=x86_64-unknown-linux-gnu ;;
      *)
        echo "orso install: unsupported Linux arch: $uname_m" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "orso install: unsupported OS: $uname_s (macOS and Linux only)" >&2
    exit 1
    ;;
esac

if [ -n "${ORSO_VERSION:-}" ]; then
  VERSION="${ORSO_VERSION#v}"
else
  # Prefer the Location header from /releases/latest (no jq).
  LOCATION=$(curl -fsSIL "https://github.com/${REPO}/releases/latest" \
    | tr -d '\r' \
    | awk 'tolower($1)=="location:" {print $2; exit}')
  case "$LOCATION" in
    */tag/v*) VERSION=${LOCATION##*/tag/v} ;;
    *)
      echo "orso install: could not resolve latest release for ${REPO}" >&2
      exit 1
      ;;
  esac
fi

ASSET="orso-v${VERSION}-${TARGET}.tar.gz"
BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "orso install: downloading ${ASSET}"
curl -fsSL "${BASE}/${ASSET}" -o "${TMP}/${ASSET}"
curl -fsSL "${BASE}/${ASSET}.sha256" -o "${TMP}/${ASSET}.sha256"

expected=$(awk '{print $1}' "${TMP}/${ASSET}.sha256")
if command -v shasum >/dev/null 2>&1; then
  actual=$(shasum -a 256 "${TMP}/${ASSET}" | awk '{print $1}')
elif command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "${TMP}/${ASSET}" | awk '{print $1}')
else
  echo "orso install: need shasum or sha256sum to verify the download" >&2
  exit 1
fi
if [ "$expected" != "$actual" ]; then
  echo "orso install: checksum mismatch for ${ASSET}" >&2
  echo "  expected: $expected" >&2
  echo "  actual:   $actual" >&2
  exit 1
fi

tar -xzf "${TMP}/${ASSET}" -C "$TMP"
BIN=$(find "$TMP" -type f -name orso | head -n 1)
if [ -z "$BIN" ]; then
  echo "orso install: archive did not contain an orso binary" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
install -m 755 "$BIN" "${INSTALL_DIR}/orso"

# macOS Gatekeeper may quarantine curl downloads; clear it when possible.
if [ "$uname_s" = Darwin ] && command -v xattr >/dev/null 2>&1; then
  xattr -d com.apple.quarantine "${INSTALL_DIR}/orso" 2>/dev/null || true
fi

echo "orso install: installed ${INSTALL_DIR}/orso (v${VERSION})"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo "orso install: add ${INSTALL_DIR} to your PATH, then run: orso serve --open"
    ;;
esac
echo "orso install: run  orso serve --open"
