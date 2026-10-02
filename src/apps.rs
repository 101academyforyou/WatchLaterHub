//! 「開啟」：加入電腦上的軟體，點一下就開啟
//!
//! - 電腦上的軟體：透過「電腦小幫手」（launcher/，Chrome Native Messaging）開啟；要先執行 ./install-launcher.sh
//! - 有專屬網址的軟體（vscode://、slack:// …）：不用小幫手，直接用網址開啟

use serde::{Deserialize, Serialize};

pub const KEY: &str = "apps";
pub const HOST: &str = "com.watchlaterhub.launcher";
/// 電腦上軟體的圖示快取：{ 路徑: data URL }（由小幫手讀取軟體本身的圖示）
pub const LOGOS_KEY: &str = "appLogos2";
/// 標題列的 5 個常用軟體（存 AppEntry 的 id）
pub const PINS_KEY: &str = "appPins";
pub const PIN_SLOTS: usize = 5;
/// 舊版快取（Mac 上可能存到透明的空白圖），啟動時刪掉
pub const OLD_LOGOS_KEY: &str = "appLogos";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    /// "app"：電腦上的軟體（target = 路徑）；"url"：用網址開啟（target = 網址）
    pub kind: String,
    pub target: String,
    /// 圖示用的官網網域（快速加入的軟體才有）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Installed {
    pub name: String,
    pub path: String,
}

/// 快速加入的軟體
pub struct Preset {
    pub name: &'static str,
    /// 圖示：官網網域（用 Google 的網站圖示服務取得各軟體官網的 favicon）
    pub icon: &'static str,
    /// 電腦上的軟體名稱（有裝桌面版就透過小幫手開啟）
    pub apps: &'static [&'static str],
    /// 沒裝桌面版時的開啟方式：專屬網址或網頁
    pub url: &'static str,
}

pub const PRESETS: [Preset; 12] = [
    Preset { name: "Claude", icon: "claude.ai", apps: &["Claude"], url: "https://claude.ai/" },
    Preset { name: "ChatGPT", icon: "chatgpt.com", apps: &["ChatGPT"], url: "https://chatgpt.com/" },
    Preset { name: "Gemini", icon: "gemini.google.com", apps: &["Gemini"], url: "https://gemini.google.com/" },
    Preset { name: "VS Code", icon: "code.visualstudio.com", apps: &["Visual Studio Code", "Code"], url: "vscode://" },
    Preset { name: "Obsidian", icon: "obsidian.md", apps: &["Obsidian"], url: "obsidian://" },
    Preset { name: "Notion", icon: "notion.so", apps: &["Notion"], url: "notion://" },
    Preset { name: "Discord", icon: "discord.com", apps: &["Discord"], url: "discord://" },
    Preset { name: "Slack", icon: "slack.com", apps: &["Slack"], url: "slack://open" },
    Preset { name: "Google Meet", icon: "meet.google.com", apps: &[], url: "https://meet.google.com/" },
    Preset { name: "Zoom", icon: "zoom.us", apps: &["zoom.us", "Zoom", "Zoom Workplace"], url: "zoommtg://" },
    Preset { name: "Teams", icon: "teams.microsoft.com", apps: &["Microsoft Teams", "Microsoft Teams (work or school)", "Microsoft Teams classic"], url: "msteams:" },
    Preset { name: "Spotify", icon: "open.spotify.com", apps: &["Spotify"], url: "spotify:" },
];

/// 電腦上的軟體名稱對到快速加入的軟體 → 官網網域（小幫手還沒回傳圖示前先用）
pub fn preset_domain(name: &str) -> Option<&'static str> {
    PRESETS.iter().find(|p| !p.icon.is_empty() && p.apps.iter().any(|a| a.eq_ignore_ascii_case(name.trim()))).map(|p| p.icon)
}

/// 網站圖示網址
pub fn icon_url(domain: &str) -> String {
    format!("https://www.google.com/s2/favicons?domain={domain}&sz=64")
}

/// 快速加入：有裝桌面版 → (app, 路徑)；否則 → (url, 網址)
pub fn resolve_preset(p: &Preset, installed: Option<&[Installed]>) -> (&'static str, String) {
    if let Some(list) = installed {
        for want in p.apps {
            if let Some(a) = list.iter().find(|a| a.name.eq_ignore_ascii_case(want)) {
                return ("app", a.path.clone());
            }
        }
    }
    ("url", p.url.to_string())
}

// ---------- 純邏輯（可在本機 cargo test） ----------

/// 網址要有通訊協定（vscode:、https: …）；不能是 javascript: 這類
pub fn valid_url(u: &str) -> bool {
    let u = u.trim();
    let Some((scheme, _)) = u.split_once(':') else { return false };
    !scheme.is_empty()
        && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        && !["javascript", "data", "file", "vbscript"].contains(&scheme.to_ascii_lowercase().as_str())
}

/// 加入一項；同一個目標不重複加。回傳是否有加入
pub fn add(list: &mut Vec<AppEntry>, name: &str, kind: &str, target: &str, id: String) -> bool {
    let (name, target) = (name.trim(), target.trim());
    if name.is_empty() || target.is_empty() || list.iter().any(|a| a.target == target) {
        return false;
    }
    if kind == "url" && !valid_url(target) {
        return false;
    }
    list.push(AppEntry { id, name: name.to_string(), kind: kind.to_string(), target: target.to_string(), icon: None });
    true
}

/// 加入快速加入的軟體（帶圖示）
pub fn add_preset(list: &mut Vec<AppEntry>, p: &Preset, installed: Option<&[Installed]>, id: String) -> bool {
    let (kind, target) = resolve_preset(p, installed);
    let ok = add(list, p.name, kind, &target, id.clone());
    if ok && !p.icon.is_empty() {
        if let Some(a) = list.iter_mut().find(|a| a.id == id) {
            a.icon = Some(p.icon.to_string());
        }
    }
    ok
}

/// 把軟體放到第 i 格；已經在別格的話移過來（同一個軟體不會出現兩次）
pub fn pin_at(pins: &mut Vec<Option<String>>, i: usize, id: &str) {
    pins.resize(PIN_SLOTS, None);
    if i >= PIN_SLOTS {
        return;
    }
    for p in pins.iter_mut() {
        if p.as_deref() == Some(id) {
            *p = None;
        }
    }
    pins[i] = Some(id.to_string());
}

/// 兩個常用格交換（拖到空格 = 移過去）
pub fn swap_pins<T>(pins: &mut [Option<T>], a: usize, b: usize) {
    if a < pins.len() && b < pins.len() {
        pins.swap(a, b);
    }
}

pub fn remove(list: &mut Vec<AppEntry>, id: &str) {
    list.retain(|a| a.id != id);
}

pub fn reorder(list: &mut Vec<AppEntry>, from: &str, to: &str, after: bool) {
    crate::videos::move_by_key(list, |a| a.id.as_str(), from, to, after);
}

/// 圖示：名稱第一個字（英文大寫）＋依名稱固定的顏色
pub fn avatar(name: &str) -> (String, u32) {
    let first = name.trim().chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    let hue = name.chars().fold(7u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32)) % 360;
    (first, hue)
}

// ---------- 畫面 ----------

mod view {
    use super::*;
    use crate::chrome::{self, to_js};
    use crate::drag::{Pos, Sortable};
    use crate::ui::{doc, el, hide, listen, on_click, spawn};
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{Element, HtmlInputElement, KeyboardEvent};

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = sendNativeMessage, catch)]
        async fn send_native_raw(host: &str, msg: JsValue) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = getPlatformInfo, catch)]
        async fn platform_info_raw() -> Result<JsValue, JsValue>;
    }

    thread_local! {
        /// 小幫手回報的已安裝軟體（None = 小幫手沒裝或連不上）
        static INSTALLED: RefCell<Option<Vec<Installed>>> = const { RefCell::new(None) };
        /// 軟體圖示快取（路徑 → data URL）
        static LOGOS: RefCell<std::collections::HashMap<String, String>> = RefCell::new(Default::default());
        /// 這次開啟新分頁已經問過小幫手的路徑（沒有圖示的不重複問）
        static ASKED: RefCell<std::collections::HashSet<String>> = RefCell::new(Default::default());
    }

    fn set_img(ico: &Element, cls: &str, src: &str) {
        ico.set_class_name(&format!("app-ico {cls}"));
        let _ = ico.remove_attribute("style");
        ico.set_inner_html(r#"<img alt="">"#);
        ico.query_selector("img").unwrap().unwrap().unchecked_into::<web_sys::HtmlImageElement>().set_src(src);
    }

    /// 圖片載得出來、而且不是整張透明的空白圖
    async fn visible(src: &str) -> bool {
        let img = web_sys::HtmlImageElement::new().unwrap();
        let p = js_sys::Promise::new(&mut |ok, bad| {
            img.set_onload(Some(ok.unchecked_ref()));
            img.set_onerror(Some(bad.unchecked_ref()));
        });
        img.set_src(src);
        if wasm_bindgen_futures::JsFuture::from(p).await.is_err() || img.natural_width() == 0 {
            return false;
        }
        let Ok(canvas) = doc().create_element("canvas") else { return true };
        let canvas: web_sys::HtmlCanvasElement = canvas.unchecked_into();
        let n = 24;
        canvas.set_width(n);
        canvas.set_height(n);
        let Some(ctx) = canvas.get_context("2d").ok().flatten() else { return true };
        let ctx: web_sys::CanvasRenderingContext2d = ctx.unchecked_into();
        if ctx.draw_image_with_html_image_element_and_dw_and_dh(&img, 0.0, 0.0, n as f64, n as f64).is_err() {
            return true;
        }
        match ctx.get_image_data(0.0, 0.0, n as f64, n as f64) {
            // 每個像素 4 個值，第 4 個是透明度
            Ok(d) => d.data().0.chunks(4).filter(|px| px[3] > 16).count() > 8,
            Err(_) => true,
        }
    }

    /// 向小幫手要軟體的圖示，存起來並換上
    fn fetch_logo(path: String) {
        if ASKED.with(|a| !a.borrow_mut().insert(path.clone())) {
            return;
        }
        spawn(async move {
            let Ok(v) = native(serde_json::json!({ "action": "icon", "path": path })).await else { return };
            let Some(icon) = v["icon"].as_str().filter(|i| i.starts_with("data:image/")).map(str::to_string) else { return };
            if !visible(&icon).await {
                return;
            }
            // 畫面可能已重畫，換掉目前所有這個軟體的圖示
            if let Ok(list) = el("apps").query_selector_all(".app-ico[data-path]") {
                for i in 0..list.length() {
                    let Some(n) = list.item(i) else { continue };
                    let e: Element = n.unchecked_into();
                    if e.get_attribute("data-path").as_deref() == Some(path.as_str()) {
                        set_img(&e, "native", &icon);
                    }
                }
            }
            // 同時有好幾個圖示回來時，都以記憶體中的完整清單寫入，避免互相覆蓋
            let all = LOGOS.with(|l| {
                l.borrow_mut().insert(path, icon);
                l.borrow().clone()
            });
            chrome::set(&[(LOGOS_KEY, to_js(&all))]).await;
        });
    }

    async fn native(msg: serde_json::Value) -> Result<serde_json::Value, String> {
        let r = send_native_raw(HOST, to_js(&msg)).await.map_err(chrome::err_msg)?;
        let v: serde_json::Value = chrome::from_js(&r).unwrap_or(serde_json::Value::Null);
        if v["ok"].as_bool() == Some(true) {
            Ok(v)
        } else {
            Err(v["error"].as_str().unwrap_or("小幫手沒有回應").to_string())
        }
    }

    async fn load() -> Vec<AppEntry> {
        chrome::get_or(KEY, vec![]).await
    }

    fn update(f: impl FnOnce(&mut Vec<AppEntry>) + 'static) {
        spawn(async move {
            let mut list = load().await;
            f(&mut list);
            chrome::set(&[(KEY, to_js(&list))]).await;
            render(&list);
            render_installed();
        });
    }

    fn new_id() -> String {
        format!("{:x}{:04x}", chrome::now() as u64, (js_sys::Math::random() * 65536.0) as u32)
    }

    fn status(msg: &str, error: bool) {
        let s = el("apps-status");
        s.set_text_content(Some(msg));
        let _ = s.class_list().toggle_with_force("err", error);
    }

    async fn launch(a: AppEntry) {
        if a.kind == "url" {
            chrome::open_in_this_tab(&a.target).await;
            return;
        }
        match native(serde_json::json!({ "action": "open", "path": a.target })).await {
            Ok(_) => status(&format!("已開啟「{}」", a.name), false),
            Err(e) if e.contains("not found") || e.contains("Specified native messaging host") => {
                status(&format!("無法開啟「{}」：還沒安裝電腦小幫手", a.name), true);
                hide("apps-add", false);
                show_helper_missing().await;
            }
            Err(e) => status(&format!("無法開啟「{}」：{e}", a.name), true),
        }
    }

    /// 軟體圖示：軟體本身的圖示 → 官網 logo → 終端機 >_ → 彩色首字母
    fn fill_icon(ico: &Element, a: &AppEntry) {
        let native_logo = if a.kind == "app" { LOGOS.with(|l| l.borrow().get(&a.target).cloned()) } else { None };
        match (native_logo, a.icon.as_deref().or_else(|| preset_domain(&a.name)), a.name.as_str()) {
            // 電腦上的軟體：用軟體本身的圖示
            (Some(src), _, _) => set_img(ico, "native", &src),
            (None, Some(d), _) => set_img(ico, "logo", &icon_url(d)),
            (None, None, "Terminal") => {
                let _ = ico.class_list().add_1("term");
                ico.set_text_content(Some(">_"));
            }
            _ => {
                let (letter, hue) = avatar(&a.name);
                ico.set_text_content(Some(&letter));
                let _ = ico.set_attribute("style", &format!("background: hsl({hue} 55% 46%)"));
            }
        }
        if a.kind == "app" {
            let _ = ico.set_attribute("data-path", &a.target);
        }
        if a.kind == "app" && LOGOS.with(|l| !l.borrow().contains_key(&a.target)) {
            fetch_logo(a.target.clone());
        }
    }

    // ---------- 標題列的 5 個常用軟體 ----------

    async fn load_pins() -> Vec<Option<String>> {
        let mut v: Vec<Option<String>> = chrome::get_or(PINS_KEY, vec![]).await;
        v.resize(PIN_SLOTS, None);
        v
    }

    async fn save_pins(v: &[Option<String>]) {
        chrome::set(&[(PINS_KEY, to_js(&v))]).await;
        render_pins().await;
    }

    async fn render_pins() {
        let (pins, list) = (load_pins().await, load().await);
        for (i, p) in pins.iter().enumerate() {
            let slot = el(&format!("app-pin-{i}"));
            slot.set_inner_html("");
            let _ = slot.remove_attribute("role");
            let _ = slot.remove_attribute("tabindex");
            // 軟體被移除了 → 這格變空
            match p.as_ref().and_then(|id| list.iter().find(|a| &a.id == id)) {
                Some(a) => {
                    let _ = slot.set_attribute("draggable", "true");
                    let _ = slot.class_list().remove_1("pin-empty");
                    let _ = slot.set_attribute("role", "button");
                    let _ = slot.set_attribute("tabindex", "0");
                    slot.set_title(&a.name);
                    let _ = slot.set_attribute("aria-label", &format!("開啟：{}", a.name));
                    let ico = doc().create_element("span").unwrap();
                    ico.set_class_name("app-ico");
                    fill_icon(&ico, a);
                    slot.append_child(&ico).unwrap();
                    let x = doc().create_element("button").unwrap();
                    x.set_class_name("pin-x");
                    x.set_text_content(Some("✕"));
                    let _ = x.set_attribute("type", "button");
                    let _ = x.set_attribute("title", "取消常用");
                    let _ = x.set_attribute("aria-label", &format!("取消常用：{}", a.name));
                    listen(&x, "click", move |e| {
                        e.stop_propagation();
                        spawn(async move {
                            let mut v = load_pins().await;
                            v[i] = None;
                            save_pins(&v).await;
                        });
                    });
                    slot.append_child(&x).unwrap();
                }
                None => {
                    let _ = slot.set_attribute("draggable", "false");
                    let _ = slot.class_list().add_1("pin-empty");
                    slot.set_text_content(Some("+"));
                    slot.set_title("把軟體拖到這裡設為常用");
                    let _ = slot.remove_attribute("aria-label");
                }
            }
        }
    }

    /// 點常用格：開啟那個軟體
    fn launch_pin(i: usize) {
        spawn(async move {
            let (pins, list) = (load_pins().await, load().await);
            if let Some(a) = pins[i].as_ref().and_then(|id| list.into_iter().find(|a| &a.id == id)) {
                launch(a).await;
            }
        });
    }

    fn setup_pins() {
        for i in 0..PIN_SLOTS {
            let slot: Element = el(&format!("app-pin-{i}")).unchecked_into();
            listen(&slot, "click", move |_| launch_pin(i));
            listen(&slot, "keydown", move |e| {
                if e.unchecked_ref::<KeyboardEvent>().key() == "Enter" {
                    launch_pin(i);
                }
            });
            // 下方的軟體拖到格子上放開 = 設為常用；常用格之間拖曳 = 交換位置；
            // 常用格也可以往下拖回軟體清單（見 tile）
            crate::drag::sortable(
                Sortable { item: slot.clone(), handle: Some(slot.clone()), zone: slot, group: "apps", id: format!("pin:{i}"), can_contain: false },
                Rc::new(move |from, _to, _pos| {
                    spawn(async move {
                        let mut v = load_pins().await;
                        match from.strip_prefix("pin:").and_then(|s| s.parse::<usize>().ok()) {
                            Some(j) => swap_pins(&mut v, j, i),
                            None => pin_at(&mut v, i, &from),
                        }
                        save_pins(&v).await;
                    })
                }),
            );
        }
    }

    fn tile(a: &AppEntry) -> Element {
        let b = doc().create_element("div").unwrap();
        b.set_class_name("app-tile");
        let _ = b.set_attribute("role", "button");
        let _ = b.set_attribute("tabindex", "0");
        let _ = b.set_attribute("title", &format!("{}\n{}", a.name, if a.kind == "url" { a.target.as_str() } else { "電腦上的軟體" }));
        b.set_inner_html(r#"<span class="app-ico"></span><span class="app-name"></span><button class="bm-del" type="button" title="移除">✕</button>"#);
        fill_icon(&b.query_selector(".app-ico").unwrap().unwrap(), a);
        b.query_selector(".app-name").unwrap().unwrap().set_text_content(Some(&a.name));
        let item = a.clone();
        listen(&b, "click", move |_| {
            let item = item.clone();
            spawn(launch(item));
        });
        let item = a.clone();
        listen(&b, "keydown", move |e| {
            if e.unchecked_ref::<KeyboardEvent>().key() == "Enter" {
                let item = item.clone();
                spawn(launch(item));
            }
        });
        let id = a.id.clone();
        listen(&b.query_selector(".bm-del").unwrap().unwrap(), "click", move |e| {
            e.stop_propagation();
            let id = id.clone();
            update(move |l| remove(l, &id));
        });
        crate::drag::sortable(
            Sortable { item: b.clone(), handle: Some(b.clone()), zone: b.clone(), group: "apps", id: a.id.clone(), can_contain: false },
            Rc::new(|from, to, pos| {
                // 從常用格拖下來：移到這個位置，並空出那一格
                match from.strip_prefix("pin:").and_then(|s| s.parse::<usize>().ok()) {
                    Some(slot) => spawn(async move {
                        let mut v = load_pins().await;
                        let Some(id) = v.get(slot).cloned().flatten() else { return };
                        v[slot] = None;
                        chrome::set(&[(PINS_KEY, to_js(&v))]).await;
                        update(move |l| {
                            if id != to {
                                reorder(l, &id, &to, pos == Pos::After);
                            }
                        });
                    }),
                    None => update(move |l| reorder(l, &from, &to, pos == Pos::After)),
                }
            }),
        );
        b
    }

    fn render(list: &[AppEntry]) {
        let grid = el("apps-grid");
        grid.set_inner_html("");
        if list.is_empty() {
            grid.set_inner_html(r#"<div class="todo-none">還沒有加入軟體，按右上「＋ 新增」</div>"#);
            spawn(render_pins());
            return;
        }
        for a in list {
            grid.append_child(&tile(a)).unwrap();
        }
        spawn(render_pins());
    }

    /// 新增區：小幫手回報的已安裝軟體（可搜尋，點一下加入）
    fn render_installed() {
        let added: Vec<String> = Vec::new();
        let box_ = el("apps-installed");
        let q = crate::ui::search_text("apps-search");
        spawn(async move {
            let mut added = added;
            added.extend(load().await.into_iter().map(|a| a.target));
            let installed = INSTALLED.with(|i| i.borrow().clone());
            box_.set_inner_html("");
            let Some(apps) = installed else { return };
            // 只在搜尋時列出符合的軟體
            if q.is_empty() {
                return;
            }
            let shown: Vec<&Installed> = apps.iter().filter(|a| crate::videos::matches(&q, &[&a.name])).take(30).collect();
            if shown.is_empty() {
                box_.set_inner_html(r#"<div class="todo-none">找不到符合的軟體</div>"#);
            }
            for a in shown {
                let b = doc().create_element("button").unwrap();
                b.set_class_name("app-pick");
                let _ = b.set_attribute("type", "button");
                let is_added = added.contains(&a.path);
                b.set_text_content(Some(&format!("{}{}", a.name, if is_added { "  ✓" } else { "" })));
                let _ = b.set_attribute("title", &a.path);
                if is_added {
                    let _ = b.class_list().add_1("on");
                }
                let (name, path) = (a.name.clone(), a.path.clone());
                listen(&b, "click", move |_| {
                    let (name, path) = (name.clone(), path.clone());
                    update(move |l| {
                        add(l, &name, "app", &path, new_id());
                    });
                });
                box_.append_child(&b).unwrap();
            }
        });
    }

    /// 沒裝小幫手：依作業系統顯示下載按鈕或安裝說明
    async fn show_helper_missing() {
        let os = match platform_info_raw().await {
            Ok(v) => js_sys::Reflect::get(&v, &"os".into()).ok().and_then(|x| x.as_string()).unwrap_or_default(),
            Err(_) => String::new(),
        };
        let dl: web_sys::HtmlAnchorElement = el("apps-helper-dl").unchecked_into();
        let (msg, steps, href, label, show_dl) = match os.as_str() {
            "mac" => (
                "要開啟電腦上的其他軟體，請先安裝「電腦小幫手」（只需一次）",
                "下載後雙擊 WatchLaterHub-Launcher.pkg 照指示安裝，裝好回來按「重新偵測」。若出現「無法驗證開發者」，到「系統設定 → 隱私權與安全性」按「強制打開」。",
                crate::config::LAUNCHER_PKG_URL,
                "下載小幫手",
                true,
            ),
            "linux" => (
                "要開啟電腦上的其他軟體，請先安裝「電腦小幫手」（只需一次）",
                "在專案資料夾執行 ./install-launcher.sh，裝好回來按「重新偵測」。",
                crate::config::LAUNCHER_HELP_URL,
                "安裝說明",
                true,
            ),
            _ => ("電腦小幫手目前只支援 Mac 與 Linux；仍可用下方的快速加入或網址開啟軟體。", "", "#", "", false),
        };
        text_of("apps-helper-msg", msg);
        text_of("apps-helper-steps", steps);
        dl.set_href(href);
        dl.set_text_content(Some(label));
        dl.set_hidden(!show_dl);
        hide("apps-helper-off", false);
    }

    fn text_of(id: &str, s: &str) {
        el(id).set_text_content(Some(s));
    }

    /// 檢查小幫手、取得已安裝軟體
    async fn check_helper() {
        match native(serde_json::json!({ "action": "list" })).await {
            Ok(v) => {
                let apps: Vec<Installed> = serde_json::from_value(v["apps"].clone()).unwrap_or_default();
                INSTALLED.with(|i| *i.borrow_mut() = Some(apps));
                hide("apps-helper-on", false);
                hide("apps-helper-off", true);
                status("", false);
            }
            Err(_) => {
                INSTALLED.with(|i| *i.borrow_mut() = None);
                hide("apps-helper-on", true);
                show_helper_missing().await;
                status("", false);
            }
        }
        render_installed();
    }

    fn add_url_from_inputs() {
        let name: HtmlInputElement = el("apps-url-name").unchecked_into();
        let url: HtmlInputElement = el("apps-url").unchecked_into();
        let (n, u) = (name.value(), url.value());
        if !valid_url(&u) {
            status("網址要包含通訊協定，例如 vscode:// 或 https://", true);
            let _ = url.focus();
            return;
        }
        let n = if n.trim().is_empty() { u.clone() } else { n };
        name.set_value("");
        url.set_value("");
        update(move |l| {
            add(l, &n, "url", &u, new_id());
        });
    }

    fn is_open() -> bool {
        !el("apps").hidden()
    }

    fn set_open(open: bool) {
        if open {
            crate::ui::close_panels_except("apps");
        }
        hide("apps", !open);
        let _ = el("apps-toggle").class_list().toggle_with_force("on", open);
        let _ = el("apps-toggle").set_attribute("aria-expanded", if open { "true" } else { "false" });
        if open {
            spawn(async {
                chrome::remove(&[OLD_LOGOS_KEY]).await;
                let logos: std::collections::HashMap<String, String> = chrome::get_or(LOGOS_KEY, Default::default()).await;
                LOGOS.with(|l| *l.borrow_mut() = logos);
                let list = load().await;
                hide("apps-add", !list.is_empty());
                render(&list);
                check_helper().await;
            });
        }
    }

    pub fn close() {
        if is_open() {
            set_open(false);
        }
    }

    pub fn start() {
        on_click("apps-toggle", || set_open(!is_open()));
        on_click("apps-close", || set_open(false));
        on_click("apps-add-btn", || {
            let show = el("apps-add").hidden();
            hide("apps-add", !show);
        });
        on_click("apps-url-add", add_url_from_inputs);
        on_click("apps-helper-recheck", || {
            spawn(async {
                text_of("apps-helper-steps", "偵測中…");
                check_helper().await;
                if INSTALLED.with(|i| i.borrow().is_some()) {
                    status("電腦小幫手已安裝，可以搜尋電腦上的軟體了", false);
                } else {
                    status("還偵測不到小幫手：裝好後若仍偵測不到，請完全關閉 Chrome 再打開", true);
                }
            })
        });
        listen(&el("apps-url"), "keydown", |e| {
            let k: &KeyboardEvent = e.unchecked_ref();
            if k.key() == "Enter" && !k.is_composing() {
                add_url_from_inputs();
            }
        });
        crate::ui::search_box("apps-search", render_installed);
        setup_pins();
        // 快速加入（有裝桌面版就用桌面版，否則用專屬網址或網頁）
        let presets = el("apps-presets");
        for (i, p) in PRESETS.iter().enumerate() {
            let b = doc().create_element("button").unwrap();
            b.set_class_name("chip preset");
            let _ = b.set_attribute("type", "button");
            b.set_inner_html(r#"<span class="plus">＋</span><span class="pico"></span><span class="pname"></span>"#);
            let pico = b.query_selector(".pico").unwrap().unwrap();
            pico.set_inner_html(r#"<img alt="">"#);
            pico.query_selector("img").unwrap().unwrap().unchecked_into::<web_sys::HtmlImageElement>().set_src(&icon_url(p.icon));
            b.query_selector(".pname").unwrap().unwrap().set_text_content(Some(p.name));
            listen(&b, "click", move |_| {
                let installed = INSTALLED.with(|x| x.borrow().clone());
                update(move |l| {
                    add_preset(l, &PRESETS[i], installed.as_deref(), new_id());
                })
            });
            presets.append_child(&b).unwrap();
        }
        // 點外面或 Esc 關閉
        let d: web_sys::EventTarget = doc().into();
        listen(&d, "mousedown", |e| {
            if !is_open() {
                return;
            }
            let inside = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                .map(|n| el("apps").contains(Some(&n)) || el("apps-toggle").contains(Some(&n)))
                .unwrap_or(false);
            if !inside {
                set_open(false);
            }
        });
        listen(&d, "keydown", |e| {
            if is_open() && e.unchecked_ref::<KeyboardEvent>().key() == "Escape" {
                set_open(false);
            }
        });
    }
}

pub use view::{close, start};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries() {
        let mut l = vec![];
        assert!(add(&mut l, "Safari", "app", "/Applications/Safari.app", "a".into()));
        assert!(!add(&mut l, "Safari 2", "app", "/Applications/Safari.app", "x".into())); // 重複
        assert!(add(&mut l, "VS Code", "url", "vscode://", "b".into()));
        assert!(!add(&mut l, "壞網址", "url", "javascript:alert(1)", "y".into()));
        assert!(!add(&mut l, "沒協定", "url", "vscode", "z".into()));
        assert!(!add(&mut l, "  ", "url", "slack://open", "w".into()));
        reorder(&mut l, "a", "b", true);
        assert_eq!(l.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        remove(&mut l, "b");
        assert_eq!(l.len(), 1);
        assert!(valid_url("spotify:") && valid_url("https://x.com") && !valid_url("file:///etc/passwd"));
        assert_eq!(avatar("visual studio").0, "V");
        assert_eq!(avatar("備忘錄").0, "備");
        assert_eq!(avatar("Notion"), avatar("Notion"));
        assert_eq!(preset_domain("Visual Studio Code"), Some("code.visualstudio.com"));
        assert_eq!(preset_domain("zoom.us"), Some("zoom.us"));
        assert_eq!(preset_domain("Terminal"), None);
        assert_eq!(preset_domain("Safari"), None);
        let inst = |n: &str, p: &str| Installed { name: n.into(), path: p.into() };
        let list = vec![inst("Claude", "/Applications/Claude.app"), inst("zoom.us", "/Applications/zoom.us.app")];
        let by = |n: &str| PRESETS.iter().find(|p| p.name == n).unwrap();
        assert_eq!(resolve_preset(by("Claude"), Some(&list)), ("app", "/Applications/Claude.app".to_string()));
        assert_eq!(resolve_preset(by("Claude"), None), ("url", "https://claude.ai/".to_string()));
        assert_eq!(resolve_preset(by("Zoom"), Some(&list)), ("app", "/Applications/zoom.us.app".to_string()));
        assert_eq!(resolve_preset(by("Google Meet"), Some(&list)).0, "url");
        assert_eq!(
            PRESETS.iter().map(|p| p.name).collect::<Vec<_>>(),
            ["Claude", "ChatGPT", "Gemini", "VS Code", "Obsidian", "Notion", "Discord", "Slack", "Google Meet", "Zoom", "Teams", "Spotify"]
        );
        let mut l2 = vec![];
        assert!(add_preset(&mut l2, by("Slack"), None, "s".into()));
        assert_eq!(l2[0].icon.as_deref(), Some("slack.com"));
        let mut pins = vec![];
        pin_at(&mut pins, 2, "a");
        assert_eq!(pins, [None, None, Some("a".to_string()), None, None]);
        pin_at(&mut pins, 0, "a"); // 移到別格
        pin_at(&mut pins, 4, "b");
        pin_at(&mut pins, 9, "c"); // 超出範圍不動
        assert_eq!(pins, [Some("a".to_string()), None, None, None, Some("b".to_string())]);
        swap_pins(&mut pins, 0, 4);
        swap_pins(&mut pins, 0, 2); // 拖到空格
        swap_pins(&mut pins, 0, 7); // 超出範圍不動
        assert_eq!(pins, [None, None, Some("b".to_string()), None, Some("a".to_string())]);
    }
}
