//! 新分頁畫面：搜尋、隨機影片、登入、管理收藏、設定精靈

use crate::chrome::{self, to_js};
use crate::store;
use crate::videos::{ago, pick_random, query, split_input, Video};
use crate::youtube::{self, CANCELLED};
use std::cell::Cell;
use std::future::Future;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Document, Element, Event, HtmlAnchorElement, HtmlButtonElement, HtmlDialogElement, HtmlElement,
    HtmlImageElement, HtmlInputElement, HtmlTextAreaElement, KeyboardEvent,
};

thread_local! {
    static LOGGING_IN: Cell<bool> = const { Cell::new(false) };
}
fn logging_in() -> bool {
    LOGGING_IN.with(|c| c.get())
}
fn set_logging_in(v: bool) {
    LOGGING_IN.with(|c| c.set(v))
}

// ---------- DOM 小工具 ----------

fn doc() -> Document {
    web_sys::window().unwrap().document().unwrap()
}
fn by_id<T: JsCast>(id: &str) -> T {
    doc().get_element_by_id(id).unwrap_or_else(|| panic!("#{id} not found")).unchecked_into()
}
fn el(id: &str) -> HtmlElement {
    by_id(id)
}
fn text(id: &str, s: &str) {
    el(id).set_text_content(Some(s));
}
fn hide(id: &str, hidden: bool) {
    el(id).set_hidden(hidden);
}
fn all(sel: &str) -> Vec<HtmlElement> {
    let list = doc().query_selector_all(sel).unwrap();
    (0..list.length()).filter_map(|i| list.item(i)).map(|n| n.unchecked_into()).collect()
}
fn dialog(id: &str) -> HtmlDialogElement {
    by_id(id)
}
fn spawn(f: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(f);
}
fn listen(target: &web_sys::EventTarget, ev: &str, f: impl FnMut(Event) + 'static) {
    let cb = Closure::<dyn FnMut(Event)>::new(f);
    target.add_event_listener_with_callback(ev, cb.as_ref().unchecked_ref()).unwrap();
    cb.forget();
}
fn on_click(id: &str, f: impl Fn() + 'static) {
    listen(&el(id), "click", move |_| f());
}
fn confirm(msg: &str) -> bool {
    web_sys::window().unwrap().confirm_with_message(msg).unwrap_or(false)
}
fn input_value(id: &str) -> String {
    by_id::<HtmlInputElement>(id).value()
}

// ---------- 搜尋 ----------

/// 送出搜尋。搜尋框空白時比照 Google 首頁：有 `empty_url` 就直接前往，沒有就把游標移回搜尋框
fn go(extra: &[(&str, &str)], empty_url: Option<&str>) {
    let q = input_value("q").trim().to_string();
    let url = if q.is_empty() {
        match empty_url {
            Some(u) => u.to_string(),
            None => {
                let _ = el("q").focus();
                return;
            }
        }
    } else {
        let mut params = vec![("q", q.as_str())];
        params.extend_from_slice(extra);
        format!("https://www.google.com/search?{}", query(&params))
    };
    let _ = web_sys::window().unwrap().location().set_href(&url);
}

// ---------- 畫面狀態 ----------

async fn render() {
    let list = store::get_all().await;
    let s = youtube::settings().await;
    let can_login = !s.connected; // 未登入時一律顯示 Continue with Google

    for b in all(".topbar .js-login") {
        b.set_hidden(!can_login || list.is_empty());
    }
    hide("avatar", !s.connected || s.channel.is_none());
    if let Some(ch) = &s.channel {
        if let Some(img) = el("avatar").query_selector("img").ok().flatten() {
            img.unchecked_into::<HtmlImageElement>().set_src(ch.thumb.as_deref().unwrap_or(""));
        }
        el("avatar").set_title(&format!("{}（YouTube 帳號）", ch.title));
    }
    text("count", &if list.is_empty() { String::new() } else { format!("({})", list.len()) });

    hide("video", list.is_empty());
    hide("empty", !list.is_empty());
    if !list.is_empty() {
        return show_random(Some(list)).await;
    }

    hide("login-big", !can_login);
    let (title, desc) = if logging_in() || (s.connected && s.synced_at == 0.0) {
        ("正在載入你的 YouTube 收藏…", "第一次同步可能需要幾秒鐘")
    } else if s.connected {
        ("你的清單裡還沒有影片", "去 YouTube 按讚幾部影片，或在「管理收藏」勾選其他播放清單。")
    } else {
        ("每開一個分頁，重溫一部你收藏的影片", "用 Google 帳號登入，自動載入你在 YouTube 按讚的影片與播放清單。")
    };
    text("empty-title", title);
    text("empty-desc", desc);
}

async fn show_random(list: Option<Vec<Video>>) {
    let list = match list {
        Some(l) => l,
        None => store::get_all().await,
    };
    let last: Option<String> = chrome::get("lastId").await;
    let Some(v) = pick_random(&list, last.as_deref(), js_sys::Math::random()) else {
        return Box::pin(render()).await;
    };
    chrome::set(&[("lastId", to_js(&v.id))]).await;

    by_id::<HtmlImageElement>("v-img").set_src(&v.thumb("hqdefault"));
    by_id::<HtmlAnchorElement>("v-link").set_href(&v.watch_url());
    by_id::<HtmlAnchorElement>("v-title").set_href(&v.watch_url());
    text("v-title", &v.title);
    text("v-author", &v.author);
}

// ---------- 一鍵登入 ----------

fn set_login_buttons_disabled(d: bool) {
    for b in all(".js-login") {
        b.unchecked_into::<HtmlButtonElement>().set_disabled(d);
    }
}

async fn login() {
    if logging_in() {
        return;
    }
    // 還沒有 Client ID：先開設定精靈（只有第一次）
    if !youtube::is_configured().await {
        return open_setup();
    }
    set_logging_in(true);
    text("login-error", "");
    hide("toast", true);
    set_login_buttons_disabled(true);
    render().await;

    if let Err(e) = youtube::connect().await {
        chrome::warn(&format!("WatchLaterHub login failed: {e}"));
        let low = e.to_lowercase();
        let msg = if e == CANCELLED || low.contains("cancel") || low.contains("did not approve") || low.contains("user closed") {
            CANCELLED.to_string()
        } else if low.contains("could not be loaded") {
            // Google 拒絕了授權要求（HTTP 400），Chrome 只會回這句模糊的錯誤
            format!(
                "Google 拒絕了登入要求，通常是 OAuth 用戶端設定不對：\n\
                 • 用戶端類型要是「網頁應用程式」\n\
                 • 已授權的重新導向 URI 要完全等於 {}\n\
                 • config.rs 的 Client ID 要屬於這個用戶端\n\
                 • 剛建立或修改的用戶端要等 5 分鐘以上才會生效",
                chrome::redirect_url()
            )
        } else {
            e
        };
        text("login-error", &msg);
        if dialog("manage").open() {
            text("yt-status", &format!("⚠ {msg}"));
        } else if el("empty").hidden() {
            // 已有影片時，空白頁的 #login-error 看不到，改用浮動提示
            show_toast(&msg);
        }
    }
    set_logging_in(false);
    set_login_buttons_disabled(false);
    render().await;
    if dialog("manage").open() {
        render_yt(false).await;
    }
}

fn show_toast(msg: &str) {
    text("toast-msg", msg);
    hide("toast", false);
}

// ---------- 設定精靈 ----------

fn open_setup() {
    text("redirect-uri", &chrome::redirect_url());
    text("setup-error", "");
    if dialog("manage").open() {
        dialog("manage").close();
    }
    let _ = dialog("setup").show_modal();
    let _ = el("client-id").focus();
}

// ---------- 管理收藏 ----------

async fn render_list() {
    let list = store::get_manual().await;
    let ul = el("list");
    ul.set_inner_html("");
    if list.is_empty() {
        ul.set_inner_html(r#"<li class="none">沒有手動加入的影片</li>"#);
        return;
    }
    for v in list.iter().rev() {
        let li: Element = doc().create_element("li").unwrap();
        li.set_inner_html(r#"<img alt=""><a target="_blank" rel="noopener"></a><button class="x" title="移除">✕</button>"#);
        let img: HtmlImageElement = li.query_selector("img").unwrap().unwrap().unchecked_into();
        img.set_src(&v.thumb("default"));
        let a: HtmlAnchorElement = li.query_selector("a").unwrap().unwrap().unchecked_into();
        a.set_href(&v.watch_url());
        a.set_text_content(Some(&v.title));
        let btn = li.query_selector("button").unwrap().unwrap();
        let id = v.id.clone();
        listen(&btn, "click", move |_| {
            let id = id.clone();
            spawn(async move {
                store::remove(&id).await;
                render_list().await;
                render().await;
            });
        });
        ul.append_child(&li).unwrap();
    }
}

async fn yt_status() {
    let s = youtube::settings().await;
    let n = store::get_synced().await.len();
    let msg = if !s.error.is_empty() {
        format!("⚠ {}", s.error)
    } else if s.synced_at > 0.0 {
        format!("已同步 {n} 部 · {}", ago((chrome::now() - s.synced_at) / 60_000.0))
    } else {
        "尚未同步".into()
    };
    text("yt-status", &msg);
}

async fn render_yt(refresh_sources: bool) {
    let s = youtube::settings().await;
    hide("yt-off", s.connected);
    hide("yt-on", !s.connected);
    if !s.connected {
        return;
    }
    text("yt-who", &s.channel.as_ref().map(|c| format!("已登入：{}", c.title)).unwrap_or_default());
    yt_status().await;

    let mut srcs: Vec<youtube::Source> = chrome::get_or("ytSourceList", vec![]).await;
    if refresh_sources || srcs.is_empty() {
        match youtube::list_sources().await {
            Ok(fresh) => {
                chrome::set(&[("ytSourceList", to_js(&fresh))]).await;
                srcs = fresh;
            }
            Err(e) => text("yt-status", &format!("⚠ {e}")),
        }
    }
    let box_ = el("yt-sources");
    box_.set_inner_html("");
    for src in srcs {
        let label = doc().create_element("label").unwrap();
        label.set_inner_html(r#"<input type="checkbox"><span></span><span class="n"></span>"#);
        let cb: HtmlInputElement = label.query_selector("input").unwrap().unwrap().unchecked_into();
        cb.set_checked(s.sources.contains(&src.id));
        cb.set_value(&src.id);
        let spans = label.query_selector_all("span").unwrap();
        spans.item(0).unwrap().set_text_content(Some(&src.title));
        spans.item(1).unwrap().set_text_content(Some(&src.count.map(|c| format!("{c} 部")).unwrap_or_default()));
        listen(&cb, "change", |_| {
            let ids: Vec<String> = all("#yt-sources input:checked")
                .into_iter()
                .map(|c| c.unchecked_into::<HtmlInputElement>().value())
                .collect();
            spawn(async move { youtube::set_sources(&ids).await });
        });
        box_.append_child(&label).unwrap();
    }
}

// ---------- 啟動 ----------

pub fn start() {
    // 圖片載入失敗時隱藏破圖示，成功時再顯示
    let d: web_sys::EventTarget = doc().into();
    for (ev, vis) in [("error", "hidden"), ("load", "")] {
        let cb = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
            if let Some(img) = e.target().and_then(|t| t.dyn_into::<HtmlImageElement>().ok()) {
                let _ = img.style().set_property("visibility", vis);
            }
        });
        d.add_event_listener_with_callback_and_bool(ev, cb.as_ref().unchecked_ref(), true).unwrap();
        cb.forget();
    }

    // 搜尋
    // 好手氣：空白時跟 Google 一樣打開塗鴉頁
    on_click("lucky", || go(&[("btnI", "1")], Some("https://www.google.com/doodles")));
    // AI 模式：有文字就直接問，空白就打開 AI 模式首頁
    on_click("ai", || go(&[("udm", "50")], Some("https://www.google.com/search?udm=50")));

    // 影片
    on_click("shuffle", || spawn(show_random(None)));

    // 登入
    for b in all(".js-login") {
        listen(&b, "click", |_| spawn(login()));
    }
    on_click("avatar", || el("open-manage").click());
    on_click("toast-close", || hide("toast", true));

    // 設定精靈
    on_click("close-setup", || dialog("setup").close());
    on_click("copy-uri", || {
        spawn(async {
            chrome::clipboard_write(&el("redirect-uri").text_content().unwrap_or_default()).await;
            text("copy-uri", "已複製 ✓");
            gloo_timeout(1500, || text("copy-uri", "複製"));
        })
    });
    on_click("save-setup", || {
        spawn(async {
            match youtube::set_client_id(&input_value("client-id")).await {
                Ok(()) => {
                    dialog("setup").close();
                    login().await;
                }
                Err(e) => text("setup-error", &e),
            }
        })
    });
    listen(&el("client-id"), "keydown", |e| {
        if e.unchecked_ref::<KeyboardEvent>().key() == "Enter" {
            el("save-setup").click();
        }
    });

    // 管理收藏
    on_click("open-manage", || {
        text("msg", "");
        let _ = dialog("manage").show_modal();
        spawn(async {
            render_list().await;
            render_yt(true).await;
        });
    });
    on_click("close-manage", || dialog("manage").close());
    listen(&el("manage"), "click", |e| {
        let is_backdrop = e.target().map(|t| t == el("manage").into()).unwrap_or(false);
        if is_backdrop {
            dialog("manage").close();
        }
    });
    on_click("add", || {
        spawn(async {
            let ta: HtmlTextAreaElement = by_id("urls");
            let lines = split_input(&ta.value());
            if lines.is_empty() {
                return;
            }
            let btn: HtmlButtonElement = by_id("add");
            btn.set_disabled(true);
            text("msg", "加入中…");
            let (added, skipped) = store::add_many(&lines).await;
            btn.set_disabled(false);
            ta.set_value("");
            let extra = if skipped > 0 { format!("，略過 {skipped} 筆（重複或無效）") } else { String::new() };
            text("msg", &format!("已加入 {added} 部{extra}"));
            render_list().await;
            render().await;
        })
    });
    on_click("yt-sync", || {
        spawn(async {
            let btn: HtmlButtonElement = by_id("yt-sync");
            btn.set_disabled(true);
            text("yt-status", "同步中…");
            let _ = youtube::sync().await;
            btn.set_disabled(false);
            yt_status().await;
            render().await;
        })
    });
    on_click("yt-disconnect", || {
        if !confirm("登出後會清除已同步的影片（手動加入的會保留）。確定嗎？") {
            return;
        }
        spawn(async {
            youtube::disconnect().await;
            render_yt(false).await;
            render().await;
        })
    });

    // 背景同步、右鍵加入、授權失效時自動更新畫面
    let cb = Closure::<dyn FnMut(JsValue, JsValue)>::new(|changes: JsValue, _area: JsValue| {
        if logging_in() {
            return;
        }
        let has = |k: &str| js_sys::Reflect::has(&changes, &k.into()).unwrap_or(false);
        if has("ytConnected") || has("ytChannel") || ((has(store::KEY) || has(store::YT_KEY)) && el("video").hidden()) {
            spawn(render());
        }
    });
    chrome::on_storage_changed(&cb);
    cb.forget();

    spawn(async {
        store::seed_defaults().await;
        render().await;
        // 保險：超過一天沒同步（例如電腦睡眠錯過排程），開新分頁時在背景補同步
        let s = youtube::settings().await;
        if s.connected && chrome::now() - s.synced_at > 24.0 * 3600e3 {
            let _ = youtube::sync().await;
        }
    });
}

fn gloo_timeout(ms: i32, f: impl FnOnce() + 'static) {
    let cb = Closure::once_into_js(f);
    let _ = web_sys::window().unwrap().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), ms);
}
