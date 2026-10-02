#!/usr/bin/env bash
# 在 Mac 上把電腦小幫手做成可以雙擊安裝的 .pkg（Apple 晶片與 Intel 通用）
#   ./launcher/build-pkg.sh            → dist/WatchLaterHub-Launcher.pkg
# 有設定 SIGN_ID（Developer ID Installer 憑證名稱）就會簽署；
# 再設定 NOTARY_PROFILE（xcrun notarytool store-credentials 建立的名稱）就會送 Apple 公證。
set -euo pipefail
cd "$(dirname "$0")/.."

[ "$(uname -s)" = "Darwin" ] || { echo "❌ 要在 Mac 上執行（需要 pkgbuild）"; exit 1; }
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v cargo >/dev/null || { echo "❌ 找不到 cargo，請先安裝 Rust：https://rustup.rs"; exit 1; }

HOST=com.watchlaterhub.launcher
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' launcher/Cargo.toml | head -1)"
OUT=dist/WatchLaterHub-Launcher.pkg
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "🔨 編譯（Apple 晶片 + Intel）…"
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --release --quiet --manifest-path launcher/Cargo.toml --target "$t"
done

ROOT="$WORK/root/Library/Application Support/WatchLaterHub"
mkdir -p "$ROOT" "$WORK/scripts"
lipo -create -output "$ROOT/launcher" \
  launcher/target/aarch64-apple-darwin/release/watchlaterhub-launcher \
  launcher/target/x86_64-apple-darwin/release/watchlaterhub-launcher
chmod 755 "$ROOT/launcher"
cp launcher/pkg/uninstall.sh "$ROOT/uninstall.sh"
chmod 755 "$ROOT/uninstall.sh"
cp launcher/pkg/postinstall "$WORK/scripts/postinstall"
chmod 755 "$WORK/scripts/postinstall"

mkdir -p dist
UNSIGNED="$WORK/unsigned.pkg"
pkgbuild --root "$WORK/root" --scripts "$WORK/scripts" \
  --identifier "$HOST" --version "$VERSION" --install-location / "$UNSIGNED"

if [ -n "${SIGN_ID:-}" ]; then
  echo "✍️  簽署：$SIGN_ID"
  productsign --sign "$SIGN_ID" "$UNSIGNED" "$OUT"
  if [ -n "${NOTARY_PROFILE:-}" ]; then
    echo "📮 送 Apple 公證…"
    xcrun notarytool submit "$OUT" --keychain-profile "$NOTARY_PROFILE" --wait
    xcrun stapler staple "$OUT"
  fi
else
  cp "$UNSIGNED" "$OUT"
  echo "⚠️  沒有簽署（未設定 SIGN_ID）：別人第一次打開會看到「無法驗證開發者」"
fi

echo "✅ $OUT（版本 $VERSION）"
