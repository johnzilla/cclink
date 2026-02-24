#!/bin/sh
# Usage: curl -fsSL https://raw.githubusercontent.com/johnzilla/cclink/main/install.sh | sh
# Override version: CCLINK_VERSION=v1.0.0 sh install.sh
set -eu

REPO="johnzilla/cclink"
BINARY="cclink"
VERSION="${CCLINK_VERSION:-latest}"

# Platform detection
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  linux)
    OS_NAME="linux"
    ;;
  darwin)
    OS_NAME="darwin"
    ;;
  *)
    printf "Error: Unsupported OS: %s\n" "$OS" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64)
    ARCH_NAME="x86_64"
    ;;
  aarch64|arm64)
    ARCH_NAME="aarch64"
    ;;
  *)
    printf "Error: Unsupported architecture: %s\n" "$ARCH" >&2
    exit 1
    ;;
esac

# macOS uses a universal binary covering all architectures
if [ "$OS_NAME" = "darwin" ]; then
  ARTIFACT="cclink-darwin-universal"
else
  ARTIFACT="cclink-${OS_NAME}-${ARCH_NAME}"
fi

# Version resolution
if [ "$VERSION" = "latest" ]; then
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' \
    | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
  if [ -z "$VERSION" ]; then
    printf "Error: Could not resolve latest version from GitHub API\n" >&2
    exit 1
  fi
fi

BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

printf "Installing %s %s for %s/%s...\n" "$BINARY" "$VERSION" "$OS_NAME" "$ARCH_NAME"

# Create a temporary directory for downloads
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# Download binary archive and checksum
printf "Downloading %s.tar.gz...\n" "$ARTIFACT"
curl -fsSL -o "${TMP_DIR}/${ARTIFACT}.tar.gz" "${BASE_URL}/${ARTIFACT}.tar.gz"
curl -fsSL -o "${TMP_DIR}/${ARTIFACT}.tar.gz.sha256" "${BASE_URL}/${ARTIFACT}.tar.gz.sha256"

# SHA256 verification
EXPECTED=$(awk '{print $1}' "${TMP_DIR}/${ARTIFACT}.tar.gz.sha256")

if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL=$(sha256sum "${TMP_DIR}/${ARTIFACT}.tar.gz" | awk '{print $1}')
  if [ "$EXPECTED" != "$ACTUAL" ]; then
    printf "Error: SHA256 checksum mismatch!\n  expected: %s\n  actual:   %s\n" "$EXPECTED" "$ACTUAL" >&2
    exit 1
  fi
  printf "SHA256 checksum OK\n"
elif command -v shasum >/dev/null 2>&1; then
  ACTUAL=$(shasum -a 256 "${TMP_DIR}/${ARTIFACT}.tar.gz" | awk '{print $1}')
  if [ "$EXPECTED" != "$ACTUAL" ]; then
    printf "Error: SHA256 checksum mismatch!\n  expected: %s\n  actual:   %s\n" "$EXPECTED" "$ACTUAL" >&2
    exit 1
  fi
  printf "SHA256 checksum OK\n"
else
  printf "Warning: Neither sha256sum nor shasum found — skipping checksum verification\n"
fi

# Extract binary
tar xzf "${TMP_DIR}/${ARTIFACT}.tar.gz" -C "${TMP_DIR}"

# Install: try ~/.local/bin first, fall back to /usr/local/bin with sudo
INSTALL_DIR="$HOME/.local/bin"
if mkdir -p "$INSTALL_DIR" 2>/dev/null; then
  cp "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
  chmod +x "${INSTALL_DIR}/${BINARY}"
else
  INSTALL_DIR="/usr/local/bin"
  printf "Cannot create ~/.local/bin, installing to %s (may require sudo)\n" "$INSTALL_DIR"
  if [ "$(id -u)" -eq 0 ]; then
    cp "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    chmod +x "${INSTALL_DIR}/${BINARY}"
  else
    sudo cp "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    sudo chmod +x "${INSTALL_DIR}/${BINARY}"
  fi
fi

# Check if INSTALL_DIR is in PATH
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*)
    ;;
  *)
    printf "\nNote: %s is not in your PATH.\n" "$INSTALL_DIR"
    printf "Add it by running:\n"
    printf "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.profile\n"
    printf "Then restart your shell or run: source ~/.profile\n"
    ;;
esac

printf "\nSuccessfully installed %s %s to %s/%s\n" "$BINARY" "$VERSION" "$INSTALL_DIR" "$BINARY"
