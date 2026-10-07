#!/usr/bin/env bash
# 安裝「電腦小幫手」：讓 WatchLaterHub 新分頁可以開啟電腦上的軟體（只需執行一次）
# 原理：Chrome 擴充功能不能直接開程式，要透過 Native Messaging 小程式代為開啟。
set -euo pipefail
cd "$(dirname "$0")"

HOST=com.watchlaterhub.launcher
EXT_ID=lnokcijoconplpecgocjgkhhagkdmcce   # manifest.json 的 key 固定了這個 ID

[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v cargo >/dev/null || { echo "❌ 找不到 cargo，請先安裝 Rust：https://rustup.rs"; exit 1; }

echo "🔨 編譯小幫手…"
cargo build --release --quiet --manifest-path launcher/Cargo.toml

case "$(uname -s)" in
  Darwin)
    BIN_DIR="$HOME/Library/Application Support/WatchLaterHub"
    MANIFEST_DIRS=(
      "$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
      "$HOME/Library/Application Support/Google/Chrome Beta/NativeMessagingHosts"
      "$HOME/Library/Application Support/Chromium/NativeMessagingHosts"
      "$HOME/Library/Application Support/Microsoft Edge/NativeMessagingHosts"
      "$HOME/Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts"
    ) ;;
  Linux)
    BIN_DIR="$HOME/.local/share/watchlaterhub"
    MANIFEST_DIRS=(
      "$HOME/.config/google-chrome/NativeMessagingHosts"
      "$HOME/.config/chromium/NativeMessagingHosts"
    ) ;;
  *) echo "❌ 目前只支援 macOS 與 Linux"; exit 1 ;;
esac

mkdir -p "$BIN_DIR"
cp launcher/target/release/watchlaterhub-launcher "$BIN_DIR/launcher"
chmod 755 "$BIN_DIR/launcher"

for dir in "${MANIFEST_DIRS[@]}"; do
  # 只裝到有安裝的瀏覽器（第一個 Chrome 一律裝）
  parent="$(dirname "$dir")"
  if [ -d "$parent" ] || [ "$dir" = "${MANIFEST_DIRS[0]}" ]; then
    mkdir -p "$dir"
    cat > "$dir/$HOST.json" <<JSON
{
  "name": "$HOST",
  "description": "WatchLaterHub 電腦小幫手：開啟電腦上的軟體",
  "path": "$BIN_DIR/launcher",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$EXT_ID/"]
}
JSON
    echo "✅ 已設定：$dir"
  fi
done

echo ""
echo "🎉 完成！回到新分頁，點「開啟」就能加入電腦上的軟體。"
echo "   （若新分頁仍顯示未安裝，請到 chrome://extensions 重新載入 WatchLaterHub）"
echo "   移除：刪除 \"$BIN_DIR\" 和上面列出的 $HOST.json"
