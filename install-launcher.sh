#!/usr/bin/env bash
# 安裝「電腦小幫手」：讓 WatchLaterHub 新分頁的「開啟」可以開啟電腦上的軟體（只需執行一次）
#
#   一行安裝（不用下載任何東西）：
#     curl -fsSL https://raw.githubusercontent.com/101academyforyou/installLauncher/main/install-launcher.sh | bash
#   或下載這個檔案後：
#     ./install-launcher.sh               安裝（下載已編譯好的小幫手，不需要 Rust）
#     ./install-launcher.sh --from-source 從原始碼編譯安裝（需要 Rust 與旁邊的 launcher/ 資料夾）
#     ./install-launcher.sh --uninstall   移除
#
# 原理：Chrome 擴充功能不能直接開程式，要透過 Native Messaging 小程式代為開啟。
# 小幫手只會列出與開啟電腦上已安裝的軟體，不會執行其他指令。
set -euo pipefail

HOST=com.watchlaterhub.launcher
EXT_ID=lnokcijoconplpecgocjgkhhagkdmcce   # WatchLaterHub manifest.json 的 key 固定了這個 ID
REPO=101academyforyou/installLauncher
BASE="${WLH_DOWNLOAD_BASE:-https://github.com/$REPO/releases/latest/download}"

MODE=install
case "${1:-}" in
  --uninstall) MODE=uninstall ;;
  --from-source) MODE=source ;;
  "") ;;
  -h|--help) sed -n '2,12p' "$0" 2>/dev/null || true; exit 0 ;;
  *) echo "❌ 不認得的參數：$1（可用 --from-source、--uninstall）"; exit 1 ;;
esac

case "$(uname -s)" in
  Darwin)
    ASSET=watchlaterhub-launcher-macos
    BIN_DIR="$HOME/Library/Application Support/WatchLaterHub"
    MANIFEST_DIRS=(
      "$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
      "$HOME/Library/Application Support/Google/Chrome Beta/NativeMessagingHosts"
      "$HOME/Library/Application Support/Chromium/NativeMessagingHosts"
      "$HOME/Library/Application Support/Microsoft Edge/NativeMessagingHosts"
      "$HOME/Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts"
    ) ;;
  Linux)
    case "$(uname -m)" in
      x86_64|amd64) ASSET=watchlaterhub-launcher-linux-x86_64 ;;
      *) ASSET="" ;;   # 其他 CPU 只能從原始碼編譯
    esac
    BIN_DIR="$HOME/.local/share/watchlaterhub"
    MANIFEST_DIRS=(
      "$HOME/.config/google-chrome/NativeMessagingHosts"
      "$HOME/.config/chromium/NativeMessagingHosts"
      "$HOME/.config/microsoft-edge/NativeMessagingHosts"
      "$HOME/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts"
    ) ;;
  *) echo "❌ 目前只支援 macOS 與 Linux"; exit 1 ;;
esac

# ---------- 移除 ----------
if [ "$MODE" = uninstall ]; then
  for dir in "${MANIFEST_DIRS[@]}"; do
    rm -f "$dir/$HOST.json"
  done
  rm -rf "$BIN_DIR"
  echo "✅ 已移除 WatchLaterHub 電腦小幫手"
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
NEW_BIN="$TMP/launcher"

sha256() {
  if command -v shasum >/dev/null; then shasum -a 256 "$1" | awk '{print $1}'; else sha256sum "$1" | awk '{print $1}'; fi
}

# ---------- 取得小幫手 ----------
download() {
  [ -n "$ASSET" ] || return 1
  command -v curl >/dev/null || { echo "❌ 找不到 curl"; return 1; }
  echo "⬇️  下載電腦小幫手…"
  curl -fsSL -o "$NEW_BIN" "$BASE/$ASSET" || return 1
  if curl -fsSL -o "$TMP/sum" "$BASE/$ASSET.sha256" 2>/dev/null; then
    [ "$(awk '{print $1}' "$TMP/sum")" = "$(sha256 "$NEW_BIN")" ] || { echo "❌ 下載的檔案檢查碼不符，請重試"; return 1; }
  fi
  return 0
}

from_source() {
  local src
  src="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || true)"
  [ -n "$src" ] && [ -f "$src/launcher/Cargo.toml" ] || { echo "❌ 找不到 launcher/ 原始碼（請在 installLauncher 資料夾裡執行）"; return 1; }
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  command -v cargo >/dev/null || { echo "❌ 找不到 cargo，請先安裝 Rust：https://rustup.rs"; return 1; }
  echo "🔨 從原始碼編譯小幫手…"
  cargo build --release --quiet --manifest-path "$src/launcher/Cargo.toml"
  cp "$src/launcher/target/release/watchlaterhub-launcher" "$NEW_BIN"
}

if [ "$MODE" = source ]; then
  from_source
elif ! download; then
  echo "⚠️  無法下載已編譯好的小幫手，改用原始碼編譯…"
  from_source || { echo "❌ 安裝失敗。請確認網路連線後再試一次。"; exit 1; }
fi

mkdir -p "$BIN_DIR"
cp "$NEW_BIN" "$BIN_DIR/launcher"
chmod 755 "$BIN_DIR/launcher"
# 用 curl 下載的檔案沒有「從網路下載」的標記；保險起見再清一次，避免 macOS 擋下
if [ "$(uname -s)" = Darwin ]; then
  xattr -d com.apple.quarantine "$BIN_DIR/launcher" 2>/dev/null || true
fi

# ---------- 告訴瀏覽器小幫手在哪裡 ----------
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
echo "🎉 完成！回到新分頁，在「開啟」按「重新偵測」，就能加入電腦上的軟體。"
echo "   （若仍偵測不到，請完全關閉瀏覽器再打開）"
echo "   移除：再執行一次並加上 --uninstall"
