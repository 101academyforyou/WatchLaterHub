//! 切換新分頁：WatchLaterHub ⇄ Chrome 原本的新分頁
//!
//! - 新分頁左上的 WatchLaterHub 圖示：改用 Chrome 原本的新分頁
//! - 工具列的 WatchLaterHub 圖示：在兩者之間切換（關閉時圖示上有「OFF」）
//!
//! 關閉時 `newtab.js` 一載入就把分頁轉到 Chrome 原本的新分頁（不等 wasm，畫面才不會閃一下）。

use crate::chrome::{self, to_js};
use crate::i18n::tr;

/// 存在 chrome.storage：true = 使用 Chrome 原本的新分頁（newtab.js 也讀這個鍵）
pub const OFF_KEY: &str = "ntpOff";
/// Chrome 原本的新分頁
pub const CHROME_NTP: &str = "chrome://new-tab-page/";

/// 是不是新分頁（Chrome 原本的，或 WatchLaterHub 的）
pub fn is_new_tab(url: &str, own_newtab: &str) -> bool {
    url.starts_with("chrome://new-tab-page") || url.starts_with("chrome://newtab") || (!own_newtab.is_empty() && url.starts_with(own_newtab))
}

pub async fn is_off() -> bool {
    chrome::get_or(OFF_KEY, false).await
}

/// 更新工具列圖示的提示文字與「OFF」標記
pub async fn refresh_action() {
    let off = is_off().await;
    chrome::action_badge(if off { "OFF" } else { "" }, "#5f6368");
    chrome::action_title(&tr(if off {
        "WatchLaterHub 已暫停，正在使用 Chrome 原本的新分頁（按一下恢復）"
    } else {
        "WatchLaterHub（按一下改用 Chrome 原本的新分頁）"
    }));
}

async fn set_off(off: bool) {
    chrome::set(&[(OFF_KEY, to_js(&off))]).await;
    refresh_action().await;
}

/// 新分頁上的圖示：改用 Chrome 原本的新分頁
pub async fn switch_to_chrome() {
    set_off(true).await;
    chrome::open_in_this_tab(CHROME_NTP).await;
}

/// 工具列圖示：切換；目前分頁是新分頁的話，順便換成另一種
pub async fn on_action_click(tab_id: f64, url: String) {
    let off = !is_off().await;
    set_off(off).await;
    if tab_id >= 0.0 && is_new_tab(&url, &chrome::extension_url("newtab.html")) {
        // 恢復時開 chrome://newtab，Chrome 會換成 WatchLaterHub 的新分頁
        chrome::tabs_set_url(tab_id, if off { CHROME_NTP } else { "chrome://newtab/" }).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_new_tabs() {
        let own = "chrome-extension://abc/newtab.html";
        assert!(is_new_tab("chrome://new-tab-page/", own));
        assert!(is_new_tab("chrome://newtab/", own));
        assert!(is_new_tab("chrome-extension://abc/newtab.html#todo", own));
        assert!(!is_new_tab("https://www.google.com/", own));
        assert!(!is_new_tab("chrome-extension://abc/reminder.html", own));
    }
}
