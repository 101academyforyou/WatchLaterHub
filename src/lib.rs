//! WatchLaterHub — Chrome 新分頁：隨機顯示一部你在 YouTube 收藏的影片 + Google 搜尋
//!
//! 所有邏輯都在 Rust 裡，編譯成 WebAssembly；
//! `extension/newtab.js` 與 `extension/background.js` 只負責載入 wasm。

pub mod config;
pub mod playlist;
pub mod videos;

mod chrome;
mod store;
mod ui;
mod youtube;

mod exports {
    use crate::{chrome, config, store, ui, youtube};
    use wasm_bindgen::prelude::*;

    const YT_PATTERNS: &[&str] = &["*://*.youtube.com/*", "*://youtu.be/*"];

    /// 新分頁進入點
    #[wasm_bindgen]
    pub fn start_newtab() {
        ui::start();
    }

    async fn sync_quietly() {
        if let Err(e) = youtube::sync().await {
            chrome::warn(&format!("WatchLaterHub sync failed: {e}"));
        }
    }

    /// 安裝／更新時：放入預設收藏、建立右鍵選單與每日同步排程
    #[wasm_bindgen]
    pub async fn on_installed() {
        store::seed_defaults().await;
        chrome::context_menu("fav-page", "加入 WatchLaterHub", "page", "documentUrlPatterns", YT_PATTERNS);
        chrome::context_menu("fav-link", "把這部影片加入 WatchLaterHub", "link", "targetUrlPatterns", YT_PATTERNS);
        chrome::alarm("sync", config::SYNC_EVERY_MIN);
        sync_quietly().await;
    }

    #[wasm_bindgen]
    pub async fn on_startup() {
        sync_quietly().await;
    }

    #[wasm_bindgen]
    pub async fn on_alarm(name: String) {
        if name == "sync" {
            sync_quietly().await;
        }
    }

    /// 右鍵選單：把目前頁面或連結的影片加入收藏
    #[wasm_bindgen]
    pub async fn on_context_menu(menu_id: String, link_url: String, page_url: String) {
        let url = if menu_id == "fav-link" { link_url } else { page_url };
        store::add_many(&[url]).await;
    }
}
