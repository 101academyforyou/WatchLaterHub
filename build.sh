#!/usr/bin/env bash
# 編譯 Rust → WebAssembly，並打包成可載入 Chrome 的 dist/WatchLaterHub.zip
set -euo pipefail
cd "$(dirname "$0")"

# 1. 需要的工具（第一次才會安裝）
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
if ! command -v rustup >/dev/null; then
  echo "❌ 找不到 rustup（Rust 還沒安裝）。請先執行："
  echo ""
  echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  echo "   source \"\$HOME/.cargo/env\""
  echo ""
  if command -v brew >/dev/null && brew list rust >/dev/null 2>&1; then
    echo "⚠️  偵測到 Homebrew 版 Rust，它無法編譯 wasm，請先 brew uninstall rust"
  fi
  exit 1
fi
rustup target add wasm32-unknown-unknown >/dev/null
command -v wasm-pack >/dev/null || cargo install wasm-pack

# 2. 編譯到 extension/pkg
wasm-pack build --release --target web --out-dir extension/pkg --no-typescript
rm -f extension/pkg/.gitignore

# 3. 打包
mkdir -p dist
rm -f dist/WatchLaterHub.zip
(cd extension && zip -qr ../dist/WatchLaterHub.zip .)

echo "✅ 完成：extension/ 可直接用「載入未封裝項目」載入；dist/WatchLaterHub.zip 可分享"
