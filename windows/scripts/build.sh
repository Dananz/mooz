#!/usr/bin/env bash
# Cross-compiles Mooz for Windows (x64) from macOS or Linux and packages
# dist/Mooz-<VERSION>-windows-x64.zip. Needs rustup, cargo-xwin
# (`cargo install cargo-xwin`) and LLVM (clang-cl, lld-link, llvm-rc), e.g.
# `brew install llvm lld`. On Windows itself, plain `cargo build --release` works.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(tr -d '[:space:]' < ../VERSION)"
TARGET=x86_64-pc-windows-msvc
for llvm in /opt/homebrew/opt/llvm/bin /opt/homebrew/opt/llvm@21/bin /usr/local/opt/llvm/bin; do
  [ -d "$llvm" ] && PATH="$llvm:$PATH"
done
export PATH

rustup target add "$TARGET" >/dev/null
cargo test --quiet
cargo xwin build --release --target "$TARGET" --xwin-arch x86_64

mkdir -p dist
ZIP="dist/Mooz-$VERSION-windows-x64.zip"
rm -f "$ZIP"
cp "target/$TARGET/release/Mooz.exe" dist/Mooz.exe
(cd dist && zip -q "$(basename "$ZIP")" Mooz.exe && rm Mooz.exe)
echo "built $ZIP"
