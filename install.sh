#!/bin/sh
# Copyright (c) 2026 Srihan Yeleswarapu.
# The Lagom License (LICENSE.md) travels with the installer and with the
# package it unpacks; installing Lagom does not change its terms.
# ---install.sh-help-start--- (the `--help` output is exactly this block)
# ---------------------------------------------------------------------------
# Lagom installer & uninstaller (macOS + Linux; Windows uses the release zip).
#
#   sh install.sh                          install the latest release
#   sh install.sh --version v0.2.0         install a pinned release
#   sh install.sh --dir ~/.lagom           install somewhere specific
#   sh install.sh --uninstall              remove it again (folder + PATH line)
#
# This script ships as a release asset: download install.sh from any
# release page's Assets section and run `sh install.sh` — no clone.
# (When the repo is public, curl -fsSL <raw install.sh URL> | sh works too.)
#
# What it does: pick the package for your OS, verify its sha256, unpack to
# ~/.lagom, sanity-check the binary (mach-O/ELF header, `lagom version`),
# and append one PATH line to your shell rc — idempotently, never twice.
# What it never does: touch ~/.cargo (a `cargo install` copy is removed
# with `cargo uninstall lagom`), or write anywhere outside the install
# dir, the rc file, and the download temp dir.
#
# On Apple Silicon vs Intel: the release binary's architecture is checked
# against this machine (an Intel mac refuses an arm64 package with a clear
# message, never a mysterious "bad CPU type" later).
# ---install.sh-help-end---
set -eu

REPO="Srihan-Yeleswarapu/lagom"
DEFAULT_DIR="$HOME/.lagom"
RC_FILES="$HOME/.zshrc $HOME/.bashrc $HOME/.profile $HOME/.config/fish/config.fish"

VERSION=""
DIR="$DEFAULT_DIR"
UNINSTALL=0

# ---------------------------------------------------------------- arguments
while [ $# -gt 0 ]; do
    case "$1" in
        --version)  [ $# -ge 2 ] || { echo "error: --version needs a value (e.g. v0.2.0)" >&2; exit 2; }
                    VERSION="$2"; shift 2 ;;
        --dir)      [ $# -ge 2 ] || { echo "error: --dir needs a value" >&2; exit 2; }
                    DIR="$2"; shift 2 ;;
        --uninstall) UNINSTALL=1; shift ;;
        -h|--help)  awk '/install\.sh-help-start/{f=1;next} /install\.sh-help-end/{f=0} f' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)          echo "error: unknown argument: $1 (try --help)" >&2; exit 2 ;;
    esac
done

# ------------------------------------------------------------ uninstall path
if [ "$UNINSTALL" = 1 ]; then
    if [ ! -d "$DIR" ]; then
        echo "nothing to uninstall — $DIR does not exist."
        exit 0
    fi
    if [ -f "$DIR/Cargo.toml" ]; then
        echo "refusing: $DIR holds Cargo.toml — a source checkout, not an install."
        exit 1
    fi
    rm -rf "$DIR"
    for rc in $RC_FILES; do
        [ -f "$rc" ] || continue
        if grep -qF "$DIR" "$rc" 2>/dev/null; then
            tmp="$rc.lagom-tmp"
            grep -vF "$DIR" "$rc" > "$tmp" || true
            if cmp -s "$rc" "$tmp"; then rm -f "$tmp"; else mv "$tmp" "$rc"; fi
            echo "cleaned the PATH line from $rc"
        fi
    done
    echo "uninstalled. Open a new terminal; 'lagom' is gone."
    exit 0
fi

# ------------------------------------------------------------------- detect
OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
    Darwin) PKG_OS="macOS" ;;
    Linux)  PKG_OS="Linux" ;;
    *) echo "error: this installer covers macOS and Linux. On Windows, download
       lagom-Windows.zip from https://github.com/$REPO/releases and follow its README." >&2; exit 1 ;;
esac
case "$ARCH" in
    arm64|aarch64) PKG_ARCH="arm64" ;;
    x86_64)        PKG_ARCH="x64" ;;
    *)             PKG_ARCH="$ARCH" ;;
esac

# ------------------------------------------------------------- get a release
# Resolve "latest" once, up front: gh takes a TAG, not the word "latest"
# (which would fail with "release not found"), and the API works authed
# even on a private repo. curl-only users resolve inside fetch instead.
if [ -z "$VERSION" ] && command -v gh >/dev/null 2>&1; then
    VERSION="$(gh api "repos/$REPO/releases/latest" --jq .tag_name 2>/dev/null)" || VERSION=""
fi

fetch() { # fetch <remote-url> <local-path>  via gh (private repos) or curl
    if command -v gh >/dev/null 2>&1 \
       && [ -n "$VERSION" ] \
       && gh release download "$VERSION" -R "$REPO" -p "$1" -O "$2" --clobber 2>/dev/null; then
        return 0
    fi
    if command -v curl >/dev/null 2>&1; then
        [ -n "$VERSION" ] || VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n1)"
        [ -n "$VERSION" ] || { echo "error: could not determine the latest release (private repo? authenticate gh: gh auth login)" >&2; exit 1; }
        curl -fsSL "https://github.com/$REPO/releases/download/$VERSION/$1" -o "$2"
    else
        echo "error: need either the gh CLI (authed, works for private repos) or curl to download." >&2
        exit 1
    fi
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
PKG="lagom-$PKG_OS.tar.gz"
echo "==> downloading ${VERSION:-latest} $PKG" >&2
fetch "$PKG"            "$TMP/$PKG"
fetch "sha256sums.txt"  "$TMP/sha256sums.txt"

# ------------------------------------------------------------------ verify
# Run in $TMP so the checksum line's filename matches the saved file.
( cd "$TMP" && grep -F "$PKG" sha256sums.txt | sha256sum -c - >/dev/null 2>&1 ) \
    || { echo "error: sha256 verification failed for $PKG — do not install a corrupted download." >&2; exit 1; }
echo "==> sha256 OK" >&2

# ------------------------------------------------- sanity-check the binary
mkdir -p "$TMP/inspect"
tar xzf "$TMP/$PKG" -C "$TMP/inspect" ./lagom 2>/dev/null || tar xzf "$TMP/$PKG" -C "$TMP/inspect"
MAGIC="$(head -c 4 "$TMP/inspect/lagom" | od -A n -t x1 | tr -d ' \n')"
case "$OS" in
    Darwin)
        case "$MAGIC" in
            cffaedfe|feedfacf) : ;;  # Mach-O 64 (either byte order)
            *) echo "error: the downloaded file is not a macOS binary (header: $MAGIC)" >&2; exit 1 ;;
        esac
        # cputype: little-endian arm64 = 0c 00 00 01 at offset 4..7; x86_64 = 07 00 00 01.
        CPU="$(dd if="$TMP/inspect/lagom" bs=1 skip=4 count=4 2>/dev/null | od -A n -t x1 | tr -d ' \n')"
        if [ "$PKG_ARCH" = "arm64" ] && [ "$CPU" != "0c000001" ]; then
            echo "error: this Lagom release was built for Intel macs, but this machine is Apple Silicon (or vice versa).
       Install Rosetta, or build from source:
         git clone https://github.com/$REPO && cd lagom && cargo install --path crates/lagom_cli" >&2
            exit 1
        fi
        if [ "$PKG_ARCH" = "x64" ] && [ "$CPU" != "07000001" ]; then
            echo "error: this Lagom release was built for Apple Silicon, but this machine is Intel.
       Build from source instead:
         git clone https://github.com/$REPO && cd lagom && cargo install --path crates/lagom_cli" >&2
            exit 1
        fi
        ;;
    Linux)
        case "$MAGIC" in
            7f454c46) : ;;  # ELF
            *) echo "error: the downloaded file is not a Linux binary (header: $MAGIC)" >&2; exit 1 ;;
        esac
        ;;
esac

# ------------------------------------------------------------------ install
mkdir -p "$DIR"
tar xzf "$TMP/$PKG" -C "$DIR"
if ! "$DIR/lagom" version >/dev/null 2>&1; then
    if [ "$OS" = "Darwin" ] && command -v xattr >/dev/null 2>&1; then
        xattr -cr "$DIR"   # Gatekeeper quarantine on unsigned binaries
        "$DIR/lagom" version >/dev/null 2>&1 || true
    fi
    "$DIR/lagom" version >/dev/null 2>&1 || { echo "error: installed but '$DIR/lagom version' would not run — see the release page's Troubleshooting." >&2; exit 1; }
fi
# head -n 1: the version output now carries the copyright line; the summary stays one line.
echo "==> installed to $DIR ($("$DIR/lagom" version | head -n 1))" >&2

# -------------------------------------------------------------- PATH setup
PATH_LINE="export PATH=\"\$PATH:$DIR\""
did_rc=""
for rc in $RC_FILES; do
    [ -f "$rc" ] || continue
    grep -qF "$DIR" "$rc" 2>/dev/null && { did_rc="already"; break; }
done
if [ -z "$did_rc" ]; then
    # Prefer zsh on macOS, bash on Linux — the file that actually loads.
    for rc in $RC_FILES; do
        if [ -f "$rc" ] || { [ "$rc" = "$HOME/.zshrc" ] && [ "$OS" = "Darwin" ]; } \
           || { [ "$rc" = "$HOME/.bashrc" ] && [ "$OS" = "Linux" ]; }; then
            mkdir -p "$(dirname "$rc")"
            printf '\n%s\n' "$PATH_LINE" >> "$rc"
            did_rc="$rc"
            break
        fi
    done
fi
case "$did_rc" in
    already) echo "==> PATH already contains $DIR (no change)" >&2 ;;
    "")      echo "note: could not find a shell rc file — add this to your shell config:
        $PATH_LINE" >&2 ;;
    *)       echo "==> added the PATH line to $did_rc (reopen your terminal, or: source $did_rc)" >&2 ;;
esac

echo "
Done. Try it:

  cd \"\$(mktemp -d)\" && $DIR/lagom new hello && cd hello && $DIR/lagom run

(after reopening the terminal, plain 'lagom' works from anywhere)
To uninstall: sh install.sh --uninstall"
