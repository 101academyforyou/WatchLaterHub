//! 「工具」：加入常用的網站或軟體，點一下就開啟
//!
//! 一律用網址開啟：網頁（https://…）或軟體的專屬網址（vscode://、slack:// …）

use serde::{Deserialize, Serialize};

pub const KEY: &str = "apps";
/// 舊版「電腦小幫手」存下的軟體圖示：{ 路徑: data URL }（舊的項目顯示用）
pub const LOGOS_KEY: &str = "appLogos2";
/// 標題列的 5 個常用軟體（存 AppEntry 的 id）
pub const PINS_KEY: &str = "appPins";
pub const PIN_SLOTS: usize = 16;
/// 舊版快取（Mac 上可能存到透明的空白圖），啟動時刪掉
pub const OLD_LOGOS_KEY: &str = "appLogos";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    /// "url"：用網址開啟（target = 網址）；"app"：舊版透過電腦小幫手開啟的軟體（已不支援）
    pub kind: String,
    pub target: String,
    /// 圖示用的官網網域（快速加入的軟體才有）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// 快速加入的軟體
pub struct Preset {
    pub name: &'static str,
    /// 圖示：官網網域（用 Google 的網站圖示服務取得各軟體官網的 favicon）
    pub icon: &'static str,
    /// 電腦上的軟體名稱（舊項目對應圖示用）
    pub apps: &'static [&'static str],
    /// 開啟方式：專屬網址或網頁
    pub url: &'static str,
}

pub const PRESETS: [Preset; 24] = [
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
    Preset { name: "Perplexity", icon: "perplexity.ai", apps: &["Perplexity"], url: "https://www.perplexity.ai/" },
    Preset { name: "Copilot", icon: "copilot.microsoft.com", apps: &["Copilot", "Microsoft Copilot"], url: "https://copilot.microsoft.com/" },
    Preset { name: "Gmail", icon: "mail.google.com", apps: &[], url: "https://mail.google.com/" },
    Preset { name: "Google Drive", icon: "drive.google.com", apps: &["Google Drive"], url: "https://drive.google.com/" },
    Preset { name: "Google Calendar", icon: "calendar.google.com", apps: &[], url: "https://calendar.google.com/" },
    Preset { name: "YouTube", icon: "youtube.com", apps: &[], url: "https://www.youtube.com/" },
    Preset { name: "GitHub", icon: "github.com", apps: &["GitHub Desktop"], url: "https://github.com/" },
    Preset { name: "Figma", icon: "figma.com", apps: &["Figma"], url: "https://www.figma.com/" },
    Preset { name: "Canva", icon: "canva.com", apps: &["Canva"], url: "https://www.canva.com/" },
    Preset { name: "LINE", icon: "line.me", apps: &["LINE"], url: "line://" },
    Preset { name: "Telegram", icon: "telegram.org", apps: &["Telegram", "Telegram Desktop"], url: "tg://" },
    Preset { name: "WhatsApp", icon: "whatsapp.com", apps: &["WhatsApp"], url: "whatsapp://" },
];

/// 軟體名稱對到快速加入的軟體 → 官網網域
pub fn preset_domain(name: &str) -> Option<&'static str> {
    PRESETS.iter().find(|p| !p.icon.is_empty() && p.apps.iter().any(|a| a.eq_ignore_ascii_case(name.trim()))).map(|p| p.icon)
}

/// 網站圖示網址
pub fn icon_url(domain: &str) -> String {
    format!("https://www.google.com/s2/favicons?domain={domain}&sz=64")
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
pub fn add_preset(list: &mut Vec<AppEntry>, p: &Preset, id: String) -> bool {
    let ok = add(list, p.name, "url", p.url, id.clone());
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
    use wasm_bindgen::JsCast;
    use web_sys::{Element, HtmlInputElement, KeyboardEvent};

    thread_local! {
        /// 舊版存下的軟體圖示（路徑 → data URL）
        static LOGOS: RefCell<std::collections::HashMap<String, String>> = RefCell::new(Default::default());
    }

    fn set_img(ico: &Element, cls: &str, src: &str) {
        ico.set_class_name(&format!("app-ico {cls}"));
        let _ = ico.remove_attribute("style");
        ico.set_inner_html(r#"<img alt="">"#);
        ico.query_selector("img").unwrap().unwrap().unchecked_into::<web_sys::HtmlImageElement>().set_src(src);
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
        // 舊版透過電腦小幫手開啟的軟體：這個功能已移除
        status(&format!("「{}」是舊版的電腦軟體項目，已不能開啟；請移除後用網址重新加入", a.name), true);
    }

    /// 軟體圖示：軟體本身的圖示 → 官網 logo → 終端機 >_ → 彩色首字母
    fn fill_icon(ico: &Element, a: &AppEntry) {
        let native_logo = if a.kind == "app" { LOGOS.with(|l| l.borrow().get(&a.target).cloned()) } else { None };
        match (native_logo, a.icon.as_deref().or_else(|| preset_domain(&a.name)), a.name.as_str()) {
            // 舊版的電腦軟體項目：用存下的軟體圖示
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
        b.set_inner_html(r#"<span class="app-ico"></span><span class="app-name" data-nt></span><button class="bm-del" type="button" title="移除">✕</button>"#);
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
            });
        }
    }

    pub fn open() {
        set_open(true);
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
        listen(&el("apps-url"), "keydown", |e| {
            let k: &KeyboardEvent = e.unchecked_ref();
            if k.key() == "Enter" && !k.is_composing() {
                add_url_from_inputs();
            }
        });
        setup_pins();
        // 快速加入（用專屬網址或網頁開啟）
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
                update(move |l| {
                    add_preset(l, &PRESETS[i], new_id());
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

pub use view::{close, open, start};

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
        assert_eq!(
            PRESETS.iter().map(|p| p.name).collect::<Vec<_>>(),
            ["Claude", "ChatGPT", "Gemini", "VS Code", "Obsidian", "Notion", "Discord", "Slack", "Google Meet", "Zoom", "Teams", "Spotify",
             "Perplexity", "Copilot", "Gmail", "Google Drive", "Google Calendar", "YouTube", "GitHub", "Figma", "Canva", "LINE", "Telegram", "WhatsApp"]
        );
        let by = |n: &str| PRESETS.iter().find(|p| p.name == n).unwrap();
        let mut l2 = vec![];
        assert!(add_preset(&mut l2, by("Slack"), "s".into()));
        assert_eq!((l2[0].kind.as_str(), l2[0].target.as_str(), l2[0].icon.as_deref()), ("url", "slack://open", Some("slack.com")));
        let s = |v: &str| Some(v.to_string());
        let mut pins = vec![];
        let at = |v: &[(usize, &str)]| {
            let mut e: Vec<Option<String>> = vec![None; PIN_SLOTS];
            for (i, x) in v {
                e[*i] = s(x);
            }
            e
        };
        pin_at(&mut pins, 2, "a");
        assert_eq!(pins, at(&[(2, "a")]));
        pin_at(&mut pins, 0, "a"); // 移到別格
        pin_at(&mut pins, 8, "b");
        pin_at(&mut pins, PIN_SLOTS, "c"); // 超出範圍不動
        assert_eq!(pins, at(&[(0, "a"), (8, "b")]));
        swap_pins(&mut pins, 0, 8);
        swap_pins(&mut pins, 0, 2); // 拖到空格
        swap_pins(&mut pins, 0, PIN_SLOTS + 3); // 超出範圍不動
        assert_eq!(pins, at(&[(2, "b"), (8, "a")]));
    }
}
