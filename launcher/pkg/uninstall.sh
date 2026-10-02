#!/bin/bash
# 移除 WatchLaterHub 電腦小幫手：sudo "/Library/Application Support/WatchLaterHub/uninstall.sh"
set -u
HOST=com.watchlaterhub.launcher
if [ "$(id -u)" != "0" ]; then
  echo "請用 sudo 執行：sudo \"$0\""
  exit 1
fi
rm -f "/Library/Google/Chrome/NativeMessagingHosts/$HOST.json" \
      "/Library/Application Support/Chromium/NativeMessagingHosts/$HOST.json" \
      "/Library/Microsoft/Edge/NativeMessagingHosts/$HOST.json"
for home in /Users/*; do
  for app in "Google/Chrome" "Google/Chrome Beta" "Chromium" "Microsoft Edge" "BraveSoftware/Brave-Browser"; do
    rm -f "$home/Library/Application Support/$app/NativeMessagingHosts/$HOST.json"
  done
done
rm -rf "/Library/Application Support/WatchLaterHub"
pkgutil --forget "$HOST" >/dev/null 2>&1 || true
echo "已移除 WatchLaterHub 電腦小幫手"
