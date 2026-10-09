//! 新分頁左側的贊助內容（自己管理的廣告）
//!
//! 內容來自網路上的設定檔（`config::ADS_URL`，放在 GitHub Pages 的 `docs/ads.json`），
//! 改了設定檔不用重新上架，使用者最慢 `REFRESH_MIN` 分鐘就會看到新內容。
//! 只把它當資料顯示（圖片、一行文字、連結），不執行任何下載來的程式，也不追蹤曝光或點擊。
//! 使用者按 ✕ 就隱藏，之後也不再下載設定檔；在「收藏」視窗底部可以重新打開。

use serde::{Deserialize, Serialize};

/// 多久重新下載一次設定檔
pub const REFRESH_MIN: f64 = 10.0;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Ad {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub text_en: String,
    #[serde(default)]
    pub url: String,
    /// 上架日期 YYYY-MM-DD（含當天），空白 = 不限
    #[serde(default)]
    pub start: String,
    /// 下架日期 YYYY-MM-DD（含當天），空白 = 不限
    #[serde(default)]
    pub end: String,
}

impl Ad {
    /// 顯示的文字（英文介面優先用 text_en）
    pub fn label(&self, en: bool) -> &str {
        if en && !self.text_en.trim().is_empty() { &self.text_en } else { &self.text }
    }
}

/// 從設定檔取出贊助內容（格式不對的略過）
pub fn parse(json: &serde_json::Value) -> Vec<Ad> {
    json["ads"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| serde_json::from_value::<Ad>(v.clone()).ok())
        .collect()
}

/// 今天（YYYY-MM-DD）可以顯示的：圖片和連結都要是 https、在上下架日期內
pub fn active<'a>(ads: &'a [Ad], today: &str) -> Vec<&'a Ad> {
    ads.iter()
        .filter(|a| a.image.starts_with("https://") && a.url.starts_with("https://") && !a.text.trim().is_empty())
        .filter(|a| a.start.is_empty() || a.start.as_str() <= today)
        .filter(|a| a.end.is_empty() || today <= a.end.as_str())
        .collect()
}

// ---------- 畫面 ----------

mod view {
    use super::*;
    use crate::chrome::{self, to_js};
    use crate::config;
    use crate::ui::{by_id, el, listen, on_click, spawn};
    use web_sys::{HtmlAnchorElement, HtmlImageElement, HtmlInputElement};

    const HIDDEN_KEY: &str = "adsHidden";
    const CACHE_KEY: &str = "adsCache";

    #[derive(Serialize, Deserialize, Default)]
    struct Cache {
        at: f64,
        ads: Vec<Ad>,
    }

    fn today() -> String {
        let d = js_sys::Date::new_0();
        format!("{:04}-{:02}-{:02}", d.get_full_year(), d.get_month() + 1, d.get_date())
    }

    fn set_visible(v: bool) {
        el("ad").set_hidden(!v);
    }

    /// 從可顯示的裡面隨機挑一則
    fn show(ads: &[Ad]) {
        let list = active(ads, &today());
        if list.is_empty() {
            return set_visible(false);
        }
        let ad = list[(js_sys::Math::random() * list.len() as f64) as usize % list.len()];
        by_id::<HtmlAnchorElement>("ad-link").set_href(&ad.url);
        by_id::<HtmlImageElement>("ad-img").set_src(&ad.image);
        el("ad-text").set_text_content(Some(ad.label(crate::i18n::is_en())));
        let _ = el("ad").set_attribute("data-id", &ad.id);
        set_visible(true);
    }

    /// 先顯示暫存的，太舊就重新下載；內容變了再換
    async fn load_and_show() {
        let cache: Cache = chrome::get_or(CACHE_KEY, Cache::default()).await;
        show(&cache.ads);
        if chrome::now() - cache.at < REFRESH_MIN * 60_000.0 {
            return;
        }
        let Ok(r) = chrome::fetch(config::ADS_URL, None, "GET").await else { return };
        if r.status >= 400 {
            return;
        }
        let ads = parse(&r.json);
        let changed = ads != cache.ads;
        chrome::set(&[(CACHE_KEY, to_js(&Cache { at: chrome::now(), ads: ads.clone() }))]).await;
        if changed {
            show(&ads);
        }
    }

    async fn set_hidden(hidden: bool) {
        chrome::set(&[(HIDDEN_KEY, to_js(&hidden))]).await;
        by_id::<HtmlInputElement>("ads-show").set_checked(!hidden);
        if hidden {
            set_visible(false);
        } else {
            load_and_show().await;
        }
    }

    pub fn start() {
        if config::ADS_URL.is_empty() {
            el("ads-opt").set_hidden(true);
            return;
        }
        on_click("ad-hide", || spawn(set_hidden(true)));
        listen(&el("ads-show"), "change", |_| {
            let show = by_id::<HtmlInputElement>("ads-show").checked();
            spawn(set_hidden(!show));
        });
        // 圖片載入失敗就整個不顯示
        listen(&el("ad-img"), "error", |_| set_visible(false));
        spawn(async {
            let hidden: bool = chrome::get_or(HIDDEN_KEY, false).await;
            by_id::<HtmlInputElement>("ads-show").set_checked(!hidden);
            if !hidden {
                load_and_show().await;
            }
        });
    }
}

pub use view::start;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_and_filters() {
        let j = json!({"ads": [
            {"id": "a", "image": "https://x/a.png", "text": "A", "url": "https://a.com", "start": "2026-10-01", "end": "2026-10-31"},
            {"id": "b", "image": "http://x/b.png", "text": "B", "url": "https://b.com"},
            {"id": "c", "image": "https://x/c.png", "text": "C", "url": "javascript:alert(1)"},
            {"id": "d", "image": "https://x/d.png", "text": "D", "text_en": "Dee", "url": "https://d.com", "end": "2026-09-30"},
            {"id": "e", "image": "https://x/e.png", "text": "E", "text_en": " ", "url": "https://e.com"},
            "壞資料"
        ]});
        let ads = parse(&j);
        assert_eq!(ads.len(), 5);
        let ids = |v: Vec<&Ad>| v.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(active(&ads, "2026-10-09")), vec!["a", "e"]);
        assert_eq!(ids(active(&ads, "2026-09-15")), vec!["d", "e"]);
        assert_eq!(ids(active(&ads, "2026-10-31")), vec!["a", "e"]);
        assert_eq!(ads[3].label(true), "Dee");
        assert_eq!(ads[4].label(true), "E");
        assert!(parse(&json!({"x": 1})).is_empty());
    }
}
