// 只負責載入 Rust 編譯出來的 WebAssembly；所有邏輯都在 src/lib.rs
// Service worker 的事件監聽必須在最上層同步註冊，所以這裡先註冊，等 wasm 載入後再交給 Rust 處理。
import init, * as wlh from "./pkg/watchlaterhub.js";

const ready = init();
const run = (f) => ready.then(f).catch((e) => console.warn("WatchLaterHub:", e));

chrome.runtime.onInstalled.addListener(() => run(() => wlh.on_installed()));
chrome.runtime.onStartup.addListener(() => run(() => wlh.on_startup()));
chrome.alarms.onAlarm.addListener((a) => run(() => wlh.on_alarm(a.name)));
chrome.contextMenus.onClicked.addListener((info, tab) =>
  run(() => wlh.on_context_menu(String(info.menuItemId), info.linkUrl || "", info.pageUrl || tab?.url || ""))
);
// 工具列圖示：切換 WatchLaterHub ⇄ Chrome 原本的新分頁
chrome.action.onClicked.addListener((tab) => run(() => wlh.on_action_click(tab?.id ?? -1, tab?.url || tab?.pendingUrl || "")));
// TODO 提醒：清單一改就重設鬧鐘（時間到由 on_alarm 跳出提醒小視窗）
chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && changes.todos) run(() => wlh.on_todos_changed());
});
