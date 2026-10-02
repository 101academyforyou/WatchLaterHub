//! WatchLaterHub — Chrome 新分頁：隨機顯示一部你在 YouTube 收藏的影片 + Google 搜尋
//!
//! 所有邏輯都在 Rust 裡，編譯成 WebAssembly；
//! `extension/newtab.js` 與 `extension/background.js` 只負責載入 wasm。

pub mod config;
pub mod playlist;
pub mod recent;
pub mod timer;
pub mod todo;
pub mod videos;
pub mod weather;

mod apps;
mod bookmarks;
mod chrome;
mod drag;
mod reminder_page;
mod store;
mod ui;
mod youtube;

mod exports {
    use crate::{chrome, store, timer, todo, ui};
    use wasm_bindgen::prelude::*;

    const YT_PATTERNS: &[&str] = &["*://*.youtube.com/*", "*://youtu.be/*"];

    /// 新分頁進入點
    #[wasm_bindgen]
    pub fn start_newtab() {
        ui::start();
    }

    /// 舊版每天自動同步的排程名稱（現在只在使用者勾選或按「立即同步」時同步）
    const OLD_SYNC_ALARM: &str = "sync";

    /// 安裝／更新時：放入預設收藏、建立右鍵選單
    #[wasm_bindgen]
    pub async fn on_installed() {
        store::seed_defaults().await;
        // 舊版的寄信功能已移除：清掉當時存的 Gmail 授權
        chrome::remove(&["gmToken", "gmEmail"]).await;
        chrome::context_menu("fav-page", "加入 WatchLaterHub", "page", "documentUrlPatterns", YT_PATTERNS);
        chrome::context_menu("fav-link", "把這部影片加入 WatchLaterHub", "link", "targetUrlPatterns", YT_PATTERNS);
        chrome::alarm_clear(OLD_SYNC_ALARM).await;
        todo::reminders::sync_alarms().await;
    }

    #[wasm_bindgen]
    pub async fn on_startup() {
        todo::reminders::sync_alarms().await;
    }

    #[wasm_bindgen]
    pub async fn on_alarm(name: String) {
        if name == OLD_SYNC_ALARM {
            chrome::alarm_clear(OLD_SYNC_ALARM).await;
        } else if name == timer::ALARM {
            timer::background::fire().await;
        } else if let Some(id) = name.strip_prefix(todo::ALARM_PREFIX) {
            todo::reminders::fire(id).await;
        }
    }

    /// TODO 清單有變動（任何分頁）：重設提醒鬧鐘
    #[wasm_bindgen]
    pub async fn on_todos_changed() {
        todo::reminders::sync_alarms().await;
    }

    /// 提醒小視窗（reminder.html）進入點
    #[wasm_bindgen]
    pub fn start_reminder() {
        crate::reminder_page::start();
    }

    /// 右鍵選單：把目前頁面或連結的影片加入收藏
    #[wasm_bindgen]
    pub async fn on_context_menu(menu_id: String, link_url: String, page_url: String) {
        let url = if menu_id == "fav-link" { link_url } else { page_url };
        store::add_many(&[url]).await;
    }
}
