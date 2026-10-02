//! 「最近」：最近瀏覽的網頁清單（來自 Chrome 瀏覽紀錄），可拖曳排序、✕ 從清單移除
//!
//! 清單自己存一份：新瀏覽的網頁放到最上面，使用者可以拖曳調整順序或移除；
//! 移除只是從這個清單拿掉，不會刪 Chrome 的瀏覽紀錄。

use serde::{Deserialize, Serialize};

pub const KEY: &str = "recent";
pub const SYNC_KEY: &str = "recentSyncedAt";
pub const REMOVED_KEY: &str = "recentRemoved";
pub const MAX: usize = 30;

/// 使用者移除過的網頁：(網址, 當時的瀏覽時間)。之後重新瀏覽（時間更新）才會再出現
pub type Removed = Vec<(String, f64)>;

/// 記下被移除的項目（只留最近 2000 筆）
pub fn remember_removed(removed: &mut Removed, items: &[Recent]) {
    for r in items {
        removed.retain(|(u, _)| u != &r.url);
        removed.push((r.url.clone(), r.at));
    }
    let n = removed.len();
    if n > 2000 {
        removed.drain(..n - 2000);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Recent {
    pub url: String,
    pub title: String,
    /// 最後瀏覽時間（毫秒時間戳）
    pub at: f64,
}

// ---------- 純邏輯（可在本機 cargo test） ----------

/// 要不要放進「最近」：只收一般網頁，略過新分頁、擴充功能頁面等
pub fn wanted(url: &str) -> bool {
    (url.starts_with("http://") || url.starts_with("https://") || url.starts_with("file://"))
        && !url.starts_with("https://www.google.com/_/chrome/newtab")
}

/// 加入新的瀏覽紀錄（`visits` 由舊到新）：已在清單的移到最上面並更新標題，新的插在最上面，最多保留 MAX 筆
pub fn apply_visits(list: &mut Vec<Recent>, visits: &[Recent], removed: &Removed) {
    let was_removed = |v: &Recent| removed.iter().any(|(u, at)| u == &v.url && *at >= v.at);
    for v in visits.iter().filter(|v| wanted(&v.url) && !was_removed(v)) {
        let title = match list.iter().position(|r| r.url == v.url) {
            // 已在清單、這次不是更新的瀏覽 → 保持原位（不打亂使用者排的順序），只補上剛載入的標題
            Some(i) if list[i].at >= v.at => {
                if list[i].title.trim().is_empty() && !v.title.trim().is_empty() {
                    list[i].title = v.title.clone();
                }
                continue;
            }
            Some(i) => {
                let old = list.remove(i);
                if v.title.trim().is_empty() { old.title } else { v.title.clone() }
            }
            None => v.title.clone(),
        };
        list.insert(0, Recent { url: v.url.clone(), title, at: v.at });
    }
    list.truncate(MAX);
}

pub fn remove(list: &mut Vec<Recent>, url: &str) {
    list.retain(|r| r.url != url);
}

pub fn reorder(list: &mut Vec<Recent>, from: &str, to: &str, after: bool) {
    crate::videos::move_by_key(list, |r| r.url.as_str(), from, to, after);
}

/// 篩選期間時，直接用瀏覽紀錄：新的在前、同網址只留一筆、略過移除過的與非一般網頁
pub fn from_history(items: Vec<Recent>, removed: &Removed) -> Vec<Recent> {
    let mut v: Vec<Recent> = items
        .into_iter()
        .filter(|r| wanted(&r.url) && !removed.iter().any(|(u, at)| u == &r.url && *at >= r.at))
        .collect();
    v.sort_by(|a, b| b.at.total_cmp(&a.at));
    let mut out: Vec<Recent> = vec![];
    for r in v {
        if !out.iter().any(|o| o.url == r.url) {
            out.push(r);
        }
    }
    out
}

/// 一組瀏覽時間中，落在 [start, end) 的最後一次
pub fn last_visit_in(times: &[f64], start: f64, end: f64) -> Option<f64> {
    times.iter().cloned().filter(|t| *t >= start && *t < end).fold(None, |m, t| Some(m.map_or(t, |x: f64| x.max(t))))
}

/// 本週從星期一開始：今天往回幾天是星期一（weekday：0 = 週日）
pub fn days_since_monday(weekday: u32) -> u32 {
    (weekday + 6) % 7
}

/// 網址的網域（顯示用）
pub fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
        .unwrap_or_default()
}

// ---------- 畫面 ----------

mod view {
    use super::*;
    use crate::chrome::{self, to_js};
    use crate::drag::{Pos, Sortable};
    use crate::ui::{doc, el, hide, listen, on_click, spawn};
    use crate::videos::ago;
    use std::cell::Cell;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::KeyboardEvent;

    thread_local! {
        static SYNCING: Cell<bool> = const { Cell::new(false) };
        /// 目前的期間篩選：(按鈕 id, 開始, 結束)；None = 全部（自己的清單）
        static FILTER: std::cell::RefCell<Option<(&'static str, f64, f64)>> = const { std::cell::RefCell::new(None) };
        /// 期間篩選中，畫面上列出的項目（「清空」用）
        static SHOWN: std::cell::RefCell<Vec<Recent>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    fn filter() -> Option<(&'static str, f64, f64)> {
        FILTER.with(|f| *f.borrow())
    }

    /// 某天 0:00 的時間戳（day_offset：0 今天、-1 昨天…）
    fn day_start(day_offset: f64) -> f64 {
        // 先移到那一天的中午（避開跨日問題），再設成 0:00
        let d = js_sys::Date::new(&JsValue::from_f64(chrome::now() + day_offset * 86_400_000.0));
        d.set_hours(0);
        d.set_minutes(0);
        d.set_seconds(0);
        d.set_milliseconds(0);
        d.get_time()
    }

    /// 自訂期間的一組欄位（日期 + 時 + 分，24 小時制）→ 時間戳；prefix 是 "rc-from" 或 "rc-to"
    fn date_input(prefix: &str) -> Option<f64> {
        let val = |suffix: &str| -> String {
            let e = el(&format!("{prefix}-{suffix}"));
            match e.dyn_ref::<web_sys::HtmlInputElement>() {
                Some(i) => i.value(),
                None => e.unchecked_into::<web_sys::HtmlSelectElement>().value(),
            }
        };
        let d = val("d");
        if d.is_empty() {
            return None;
        }
        let t = js_sys::Date::new(&JsValue::from_str(&format!("{d}T{}:{}:00", val("h"), val("m")))).get_time();
        (!t.is_nan()).then_some(t)
    }

    /// 把時間戳填進一組欄位
    fn set_date_input(prefix: &str, ms: f64) {
        let d = js_sys::Date::new(&JsValue::from_f64(ms));
        el(&format!("{prefix}-d"))
            .unchecked_into::<web_sys::HtmlInputElement>()
            .set_value(&format!("{:04}-{:02}-{:02}", d.get_full_year(), d.get_month() + 1, d.get_date()));
        el(&format!("{prefix}-h")).unchecked_into::<web_sys::HtmlSelectElement>().set_value(&format!("{:02}", d.get_hours()));
        el(&format!("{prefix}-m")).unchecked_into::<web_sys::HtmlSelectElement>().set_value(&format!("{:02}", d.get_minutes()));
    }

    /// 時 00–23、分 00–59 的下拉選單
    fn fill_time_selects() {
        for prefix in ["rc-from", "rc-to"] {
            for (suffix, n) in [("h", 24), ("m", 60)] {
                let sel = el(&format!("{prefix}-{suffix}"));
                for i in 0..n {
                    let o: web_sys::HtmlOptionElement = doc().create_element("option").unwrap().unchecked_into();
                    o.set_value(&format!("{i:02}"));
                    o.set_text(&format!("{i:02}"));
                    sel.append_child(&o).unwrap();
                }
            }
        }
    }

    fn set_filter(f: Option<(&'static str, f64, f64)>) {
        FILTER.with(|x| *x.borrow_mut() = f);
        for id in ["rf-today", "rf-yesterday", "rf-before", "rf-week", "rf-custom"] {
            let _ = el(id).class_list().toggle_with_force("on", f.is_some_and(|(k, _, _)| k == id));
        }
        spawn(async { render_async(load().await).await });
    }

    /// 點期間按鈕：再點一次同一個 = 取消篩選
    fn pick_filter(id: &'static str) {
        if filter().is_some_and(|(k, _, _)| k == id) && id != "rf-custom" {
            hide("rc-row", true);
            return set_filter(None);
        }
        let now = chrome::now();
        match id {
            "rf-today" => set_filter(Some((id, day_start(0.0), now))),
            "rf-yesterday" => set_filter(Some((id, day_start(-1.0), day_start(0.0)))),
            "rf-before" => set_filter(Some((id, day_start(-2.0), day_start(-1.0)))),
            "rf-week" => {
                let back = days_since_monday(js_sys::Date::new_0().get_day()) as f64;
                set_filter(Some((id, day_start(-back), now)))
            }
            _ => {
                // 自訂：打開日期列（再點一次收起並取消）
                let show = el("rc-row").hidden();
                hide("rc-row", !show);
                if show {
                    if el("rc-from-d").unchecked_into::<web_sys::HtmlInputElement>().value().is_empty() {
                        // 預設：過去 24 小時
                        set_date_input("rc-from", now - 86_400_000.0);
                        set_date_input("rc-to", now);
                    }
                    let _ = el("rc-from-d").focus();
                } else if filter().is_some_and(|(k, _, _)| k == "rf-custom") {
                    set_filter(None);
                }
                return;
            }
        }
        hide("rc-row", true);
    }

    fn apply_custom() {
        let (Some(a), Some(b)) = (date_input("rc-from"), date_input("rc-to")) else { return };
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        // 結束時間包含那一分鐘
        set_filter(Some(("rf-custom", a, b + 60_000.0)));
    }

    async fn load() -> Vec<Recent> {
        chrome::get_or(KEY, vec![]).await
    }

    async fn save(list: &[Recent]) {
        chrome::set(&[(KEY, to_js(list))]).await;
    }

    /// 把上次同步之後的瀏覽紀錄併進清單
    async fn sync() -> Vec<Recent> {
        let mut list = load().await;
        if SYNCING.with(|s| s.replace(true)) {
            return list;
        }
        let since: f64 = chrome::get_or(SYNC_KEY, 0.0).await;
        // 第一次：抓最近 7 天
        // 往回多抓 1 分鐘，補上剛才還沒載入完的標題
        let start = if since > 0.0 { since - 60_000.0 } else { chrome::now() - 7.0 * 86_400_000.0 };
        let mut items = chrome::history_since(start, 200).await;
        items.sort_by(|a, b| a.last_visit.total_cmp(&b.last_visit));
        let visits: Vec<Recent> =
            items.into_iter().map(|h| Recent { url: h.url, title: h.title, at: h.last_visit }).collect();
        let newest = visits.last().map(|v| v.at).unwrap_or(since);
        if !visits.is_empty() {
            let removed: Removed = chrome::get_or(REMOVED_KEY, vec![]).await;
            apply_visits(&mut list, &visits, &removed);
            save(&list).await;
        }
        chrome::set(&[(SYNC_KEY, to_js(&newest.max(since)))]).await;
        SYNCING.with(|s| s.set(false));
        list
    }

    fn update(f: impl FnOnce(&mut Vec<Recent>) + 'static) {
        spawn(async move {
            let mut list = load().await;
            let before = list.clone();
            f(&mut list);
            // 被拿掉的項目記下來，避免下次同步又加回來
            let gone: Vec<Recent> = before.into_iter().filter(|b| !list.iter().any(|r| r.url == b.url)).collect();
            if !gone.is_empty() {
                let mut removed: Removed = chrome::get_or(REMOVED_KEY, vec![]).await;
                remember_removed(&mut removed, &gone);
                chrome::set(&[(REMOVED_KEY, to_js(&removed))]).await;
            }
            save(&list).await;
            render_async(list).await;
        });
    }

    /// 重畫（先查哪些已經是書籤）
    async fn render_async(list: Vec<Recent>) {
        let marked = crate::bookmarks::all_urls().await;
        // 有選期間：直接查那段時間的瀏覽紀錄
        let list = match filter() {
            Some((_, start, end)) => {
                let removed: Removed = chrome::get_or(REMOVED_KEY, vec![]).await;
                // chrome.history.search 會回傳「這段期間曾瀏覽過」的網頁，但時間是最後一次瀏覽（可能在期間之後）
                // → 期間外的再查每次瀏覽時間，只留真的在期間內看過的，時間用期間內最後一次
                let mut out = vec![];
                for h in chrome::history_range(start, end, 300).await {
                    if !wanted(&h.url) {
                        continue;
                    }
                    let at = if h.last_visit >= start && h.last_visit < end {
                        Some(h.last_visit)
                    } else {
                        last_visit_in(&chrome::history_visit_times(&h.url).await, start, end)
                    };
                    if let Some(at) = at {
                        out.push(Recent { url: h.url, title: h.title, at });
                    }
                }
                from_history(out, &removed)
            }
            None => list,
        };
        if filter().is_some() {
            SHOWN.with(|x| *x.borrow_mut() = list.clone());
        }
        render(&list, &marked);
    }

    fn render(list: &[Recent], marked: &std::collections::HashSet<String>) {
        let ul = el("recent-list");
        ul.set_inner_html("");
        let filtered = filter().is_some();
        if list.is_empty() {
            ul.set_inner_html(if filtered { r#"<li class="todo-none">這段期間沒有瀏覽紀錄</li>"# } else { r#"<li class="todo-none">還沒有瀏覽紀錄</li>"# });
            return;
        }
        let q = crate::ui::search_text("recent-search");
        let shown: Vec<&Recent> = list.iter().filter(|r| crate::videos::matches(&q, &[&r.title, &r.url])).collect();
        if shown.is_empty() {
            ul.set_inner_html(r#"<li class="todo-none">找不到符合的網頁</li>"#);
            return;
        }
        let now = chrome::now();
        for r in shown {
            let label = if r.title.trim().is_empty() { r.url.as_str() } else { r.title.as_str() };
            let li = doc().create_element("li").unwrap();
            li.set_class_name("bm-row recent-row");
            if !filtered {
                li.append_child(&crate::drag::grip()).unwrap();
            }
            let a = crate::bookmarks::link_anchor(&r.url, label);
            let meta = doc().create_element("small").unwrap();
            meta.set_class_name("recent-meta");
            let host = host_of(&r.url);
            let when = ago((now - r.at) / 60_000.0);
            meta.set_text_content(Some(&if host.is_empty() { when } else { format!("{host} · {when}") }));
            a.append_child(&meta).unwrap();
            li.append_child(&a).unwrap();

            // ☆ 加入書籤（已是書籤顯示 ★，點了可以編輯）
            let star = doc().create_element("button").unwrap();
            let saved = marked.contains(&r.url);
            star.set_class_name(if saved { "recent-star on" } else { "recent-star" });
            star.set_text_content(Some(if saved { "★" } else { "☆" }));
            let _ = star.set_attribute("type", "button");
            let _ = star.set_attribute("title", if saved { "已加入書籤（點一下編輯）" } else { "加入書籤" });
            let _ = star.set_attribute("aria-label", &format!("{}：{label}", if saved { "編輯書籤" } else { "加入書籤" }));
            let (t, u) = (label.to_string(), r.url.clone());
            listen(&star, "click", move |e| {
                e.prevent_default();
                e.stop_propagation();
                let (t, u) = (t.clone(), u.clone());
                spawn(async move { crate::bookmarks::open_editor_for(Some((t, u))).await });
            });
            li.append_child(&star).unwrap();

            let x = doc().create_element("button").unwrap();
            x.set_class_name("bm-del");
            x.set_text_content(Some("✕"));
            let _ = x.set_attribute("type", "button");
            let _ = x.set_attribute("title", "從清單移除（不會刪除瀏覽紀錄）");
            let _ = x.set_attribute("aria-label", &format!("從最近移除：{label}"));
            let (url, item) = (r.url.clone(), (*r).clone());
            listen(&x, "click", move |e| {
                e.prevent_default();
                e.stop_propagation();
                if filter().is_some() {
                    // 期間篩選中：記為移除，這次瀏覽不再顯示（也從自己的清單拿掉）
                    let item = item.clone();
                    spawn(async move {
                        let mut removed: Removed = chrome::get_or(REMOVED_KEY, vec![]).await;
                        remember_removed(&mut removed, std::slice::from_ref(&item));
                        chrome::set(&[(REMOVED_KEY, to_js(&removed))]).await;
                        let mut l = load().await;
                        remove(&mut l, &item.url);
                        save(&l).await;
                        render_async(l).await;
                    });
                } else {
                    let url = url.clone();
                    update(move |l| remove(l, &url));
                }
            });
            li.append_child(&x).unwrap();

            crate::drag::sortable(
                // 期間篩選中依時間排列，不能拖曳
                Sortable { item: li.clone(), handle: (!filtered).then(|| li.clone()), zone: li.clone(), group: "recent", id: r.url.clone(), can_contain: false },
                Rc::new(|from, to, pos| update(move |l| reorder(l, &from, &to, pos == Pos::After))),
            );
            ul.append_child(&li).unwrap();
        }
    }

    fn is_open() -> bool {
        !el("recent").hidden()
    }

    /// 被別的面板打開時關掉自己
    pub fn close() {
        if is_open() {
            set_open(false);
        }
    }

    fn set_open(open: bool) {
        if open {
            crate::ui::close_panels_except("recent");
        }
        hide("recent", !open);
        let _ = el("recent-toggle").class_list().toggle_with_force("on", open);
        let _ = el("recent-toggle").set_attribute("aria-expanded", if open { "true" } else { "false" });
        if open {
            let _ = el("recent-search").focus();
            spawn(async { render_async(sync().await).await });
        }
    }

    pub fn start() {
        on_click("recent-toggle", || set_open(!is_open()));
        on_click("recent-close", || set_open(false));
        on_click("recent-clear", || {
            if filter().is_none() {
                return update(|l| l.clear());
            }
            // 期間篩選中：把這段期間列出的全部清掉（記為移除，不會刪除瀏覽紀錄）
            let shown = SHOWN.with(|x| x.borrow().clone());
            spawn(async move {
                let mut removed: Removed = chrome::get_or(REMOVED_KEY, vec![]).await;
                remember_removed(&mut removed, &shown);
                chrome::set(&[(REMOVED_KEY, to_js(&removed))]).await;
                let mut l = load().await;
                l.retain(|r| !shown.iter().any(|s| s.url == r.url));
                save(&l).await;
                render_async(l).await;
            });
        });
        for id in ["rf-today", "rf-yesterday", "rf-before", "rf-week", "rf-custom"] {
            on_click(id, move || pick_filter(id));
        }
        on_click("rc-go", apply_custom);
        fill_time_selects();
        crate::ui::search_box("recent-search", || spawn(async { render_async(load().await).await }));

        let d: web_sys::EventTarget = doc().into();
        listen(&d, "mousedown", |e| {
            if !is_open() {
                return;
            }
            let inside = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                .map(|n| {
                    el("recent").contains(Some(&n))
                        || el("recent-toggle").contains(Some(&n))
                        || el("bm-dialog").contains(Some(&n)) // 在「加入書籤」視窗裡操作時不要關掉
                })
                .unwrap_or(false);
            if !inside {
                set_open(false);
            }
        });
        listen(&d, "keydown", |e| {
            if is_open() && e.unchecked_ref::<KeyboardEvent>().key() == "Escape" {
                set_open(false);
                let _ = el("recent-toggle").focus();
            }
        });

        // 開著的時候有新的瀏覽就更新（等一下讓網頁標題載入）
        chrome::on_history_visited(|| {
            for ms in [300, 2500] {
                crate::ui::timeout(ms, || {
                    if is_open() {
                        spawn(async { render_async(sync().await).await });
                    }
                });
            }
        });
        // 其他分頁改了清單
        let cb = Closure::<dyn FnMut(JsValue, JsValue)>::new(|changes: JsValue, _area: JsValue| {
            if is_open()
                && js_sys::Reflect::has(&changes, &KEY.into()).unwrap_or(false)
                && doc().query_selector("#recent-list .dragging").ok().flatten().is_none()
            {
                spawn(async { render_async(load().await).await });
            }
        });
        chrome::on_storage_changed(&cb);
        // 書籤有變動（例如剛按 ☆ 加入）：更新 ☆／★
        chrome::on_bookmarks_changed(|| {
            if is_open() {
                spawn(async { render_async(load().await).await });
            }
        });
        cb.forget();

        // 背景先同步一次，打開時就是新的
        spawn(async {
            sync().await;
        });
    }
}

pub use view::{close, start};

#[cfg(test)]
mod tests {
    use super::*;

    fn r(url: &str, title: &str, at: f64) -> Recent {
        Recent { url: url.into(), title: title.into(), at }
    }
    fn urls(l: &[Recent]) -> String {
        l.iter().map(|x| x.url.trim_start_matches("https://")).collect::<Vec<_>>().join(",")
    }

    #[test]
    fn visits_and_edits() {
        let none: Removed = vec![];
        let mut l = vec![];
        apply_visits(&mut l, &[r("https://a", "A", 1.0), r("chrome://settings", "S", 2.0), r("https://b", "B", 3.0)], &none);
        assert_eq!(urls(&l), "b,a");
        // 使用者調整順序、移除
        reorder(&mut l, "https://b", "https://a", true);
        assert_eq!(urls(&l), "a,b");
        // 再次瀏覽 b（較新）→ 移到最上面；標題空白時保留舊標題
        apply_visits(&mut l, &[r("https://b", "", 5.0), r("https://c", "C", 6.0)], &none);
        assert_eq!(urls(&l), "c,b,a");
        assert_eq!(l[1].title, "B");
        // 同一次瀏覽、標題後來才載入 → 補上標題但不移動
        apply_visits(&mut l, &[r("https://q", "", 7.0)], &none);
        apply_visits(&mut l, &[r("https://c", "C2", 6.0), r("https://q", "Q", 7.0)], &none);
        assert_eq!(urls(&l), "q,c,b,a");
        assert_eq!((l[0].title.as_str(), l[1].title.as_str()), ("Q", "C"));
        remove(&mut l, "https://q");
        assert_eq!(l[1].at, 5.0);
        // 舊的紀錄不會打亂順序
        apply_visits(&mut l, &[r("https://a", "A", 1.0)], &none);
        assert_eq!(urls(&l), "c,b,a");
        remove(&mut l, "https://c");
        assert_eq!(urls(&l), "b,a");
        // 上限
        let many: Vec<_> = (0..40).map(|i| r(&format!("https://x{i}"), "x", 10.0 + i as f64)).collect();
        apply_visits(&mut l, &many, &none);
        assert_eq!(l.len(), MAX);
        assert_eq!(l[0].url, "https://x39");
        assert_eq!(host_of("https://www.youtube.com/watch?v=1"), "youtube.com");

        // 移除過的：同一次瀏覽不再加回，重新瀏覽才會出現
        let mut l = vec![r("https://a", "A", 1.0)];
        let mut removed = vec![];
        remember_removed(&mut removed, &l);
        remove(&mut l, "https://a");
        apply_visits(&mut l, &[r("https://a", "A", 1.0)], &removed);
        assert!(l.is_empty());
        apply_visits(&mut l, &[r("https://a", "A", 2.0)], &removed);
        assert_eq!(urls(&l), "a");

        // 期間篩選：新的在前、同網址只留最新一次、略過移除過的
        let removed = vec![("https://c".to_string(), 3.0)];
        let h = from_history(vec![r("https://a", "A", 1.0), r("https://b", "B", 5.0), r("https://a", "A", 4.0), r("https://c", "C", 3.0), r("chrome://x", "X", 9.0)], &removed);
        assert_eq!(urls(&h), "b,a");
        assert_eq!(h[1].at, 4.0);
        assert_eq!((days_since_monday(1), days_since_monday(0), days_since_monday(5)), (0, 6, 4));
        // 昨天看過、今天又看：期間「昨天」要用昨天那次
        assert_eq!(last_visit_in(&[10.0, 25.0, 140.0], 0.0, 100.0), Some(25.0));
        assert_eq!(last_visit_in(&[140.0], 0.0, 100.0), None);
        assert_eq!(last_visit_in(&[100.0], 0.0, 100.0), None);
    }
}
