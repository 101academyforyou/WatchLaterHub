// 只負責載入 Rust 編譯出來的 WebAssembly；所有邏輯都在 src/*.rs
// 例外：使用者按工具列圖示改用 Chrome 原本的新分頁時（src/ntp.rs），這裡不載入 wasm 直接轉過去，畫面才不會閃一下
import init, { start_newtab } from "./pkg/watchlaterhub.js";

const { ntpOff } = await chrome.storage.local.get("ntpOff");
if (ntpOff) {
  const tab = await chrome.tabs.getCurrent();
  chrome.tabs.update(tab.id, { url: "chrome://new-tab-page/" });
} else {
  await init();
  start_newtab();
}
