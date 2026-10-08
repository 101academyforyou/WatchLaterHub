//! TODO 清單：頂列「TODO」按鈕打開的下拉清單，存在 chrome.storage

use serde::{Deserialize, Serialize};

pub const KEY: &str = "todos";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Todo {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub done: bool,
    /// 提醒時間（毫秒時間戳）
    #[serde(rename = "remindAt", default, skip_serializing_if = "Option::is_none")]
    pub remind_at: Option<f64>,
    /// 這次的提醒已經跳出過了
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub notified: bool,
    /// 優先順序："high"／"mid"／"low"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// 任務矩陣：1 重要且緊急、2 重要不緊急、3 緊急不重要、4 不緊急不重要
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quadrant: Option<u8>,
    /// 有筆記或圖片（內容另外存在 `note_key(id)`，清單本身保持很小）
    #[serde(rename = "hasNote", default, skip_serializing_if = "std::ops::Not::not")]
    pub has_note: bool,
}

/// 每個待辦事項的筆記（📝 小視窗）
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Note {
    #[serde(default)]
    pub text: String,
    /// 舊版：另外列在下方的圖片（data: 網址）。打開時會搬進 `html`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    /// 筆記內容（文字、圖片和附件混排的 HTML：只有文字、換行、<img> 和附件標籤）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub html: String,
    /// 筆記裡的附件 id；檔案本身另外存在 `file_key(id)`，打字存檔時才不用每次重寫
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

/// 筆記裡的附件（任何類型的檔案）
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NoteFile {
    pub name: String,
    #[serde(rename = "type")]
    pub mime: String,
    /// data: 網址
    pub data: String,
}

/// 檔案存在 chrome.storage 的鍵名
pub fn file_key(fid: &str) -> String {
    format!("todoFile:{fid}")
}

/// 單一檔案的大小上限
pub const MAX_FILE_BYTES: f64 = 30.0 * 1024.0 * 1024.0;

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// 1234567 → "1.2 MB"
pub fn size_label(bytes: f64) -> String {
    if bytes >= 1024.0 * 1024.0 {
        format!("{:.1} MB", bytes / 1024.0 / 1024.0)
    } else {
        format!("{} KB", (bytes / 1024.0).ceil().max(1.0))
    }
}

fn ext_of(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}

/// 依檔案類型挑圖示
pub fn file_icon(name: &str, mime: &str) -> &'static str {
    let ext = ext_of(name);
    match () {
        _ if mime == "application/pdf" || ext == "pdf" => "📄",
        _ if mime.starts_with("image/") => "🖼",
        _ if mime.starts_with("video/") => "🎬",
        _ if mime.starts_with("audio/") => "🎵",
        _ if matches!(ext.as_str(), "zip" | "rar" | "7z" | "gz" | "tar" | "bz2" | "xz") => "🗜",
        _ if matches!(ext.as_str(), "xls" | "xlsx" | "csv" | "numbers" | "ods") => "📊",
        _ if matches!(ext.as_str(), "ppt" | "pptx" | "key" | "odp") => "📽",
        _ if mime.starts_with("text/") || matches!(ext.as_str(), "doc" | "docx" | "pages" | "odt" | "rtf" | "md" | "txt") => "📝",
        _ => "📎",
    }
}

/// 瀏覽器能直接在分頁裡顯示的類型（其他類型點了改成下載）
pub fn viewable(name: &str, mime: &str) -> bool {
    mime == "application/pdf"
        || mime == "application/json"
        || ["text/", "image/", "video/", "audio/"].iter().any(|p| mime.starts_with(p))
        || ext_of(name) == "pdf"
}

/// 可以直接放在筆記裡顯示的圖片格式（其他圖片當成附件）
pub fn inline_image(mime: &str) -> bool {
    matches!(mime, "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/bmp" | "image/avif" | "image/svg+xml")
}

/// 筆記裡代表一個附件的標籤（整塊不能編輯，點一下打開或下載）
pub fn file_chip(fid: &str, name: &str, bytes: f64, mime: &str) -> String {
    let fid: String = fid.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!(
        r#"<span class="note-file" contenteditable="false" data-fid="{fid}"><span class="nf-ico">{}</span><span class="nf-name">{}</span><span class="nf-size">{}</span></span>"#,
        file_icon(name, mime),
        esc_html(name),
        size_label(bytes)
    )
}

/// HTML 裡用到的檔案 id（依出現順序、不重複）
pub fn fids_in(html: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for part in html.split("data-fid=\"").skip(1) {
        let id: String = part.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if !id.is_empty() && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

impl Note {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.images.is_empty() && !self.html.contains("<img") && self.files.is_empty()
    }

    /// 編輯區要顯示的 HTML（舊版只有文字和下方圖片的筆記，轉成混排）
    pub fn to_html(&self) -> String {
        if !self.html.is_empty() {
            return self.html.clone();
        }
        let esc = self.text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        let mut out = esc.replace('\n', "<br>");
        for src in &self.images {
            out.push_str(&format!("<img src=\"{}\">", src.replace('"', "")));
        }
        out
    }
}

/// 筆記存在 chrome.storage 的鍵名
pub fn note_key(id: &str) -> String {
    format!("todoNote:{id}")
}

/// 清單變動後被刪掉的項目（要一起刪掉它們的筆記）
pub fn removed_ids(before: &[Todo], after: &[Todo]) -> Vec<String> {
    before.iter().filter(|b| !after.iter().any(|a| a.id == b.id)).map(|b| b.id.clone()).collect()
}

/// 改主題（空白不改）
pub fn set_title(list: &mut [Todo], id: &str, text: &str) {
    let text = text.trim();
    if let Some(t) = list.iter_mut().find(|t| t.id == id).filter(|_| !text.is_empty()) {
        t.text = text.to_string();
    }
}

pub fn set_has_note(list: &mut [Todo], id: &str, has: bool) {
    if let Some(t) = list.iter_mut().find(|t| t.id == id) {
        t.has_note = has;
    }
}

/// 任務矩陣：(編號, 名稱, 建議)
pub const QUADRANTS: [(u8, &str, &str); 4] = [
    (1, "重要且緊急", "立即去做"),
    (2, "重要不緊急", "排時間做"),
    (3, "緊急不重要", "盡快處理或交給別人"),
    (4, "不緊急不重要", "少做或刪除"),
];

pub fn quadrant_name(q: u8) -> Option<&'static str> {
    QUADRANTS.iter().find(|(n, _, _)| *n == q).map(|(_, name, _)| *name)
}

/// 優先順序：(值, 圓點, 名稱)
pub const PRIORITIES: [(&str, &str, &str); 3] = [("high", "🔴", "高優先"), ("mid", "🟡", "中優先"), ("low", "🟢", "低優先")];

fn priority_info(p: &str) -> Option<(&'static str, &'static str, &'static str)> {
    PRIORITIES.iter().copied().find(|(v, _, _)| *v == p)
}

/// 提醒用的 alarm／通知名稱前綴
pub const ALARM_PREFIX: &str = "todo:";

// ---------- 純邏輯（可在本機 cargo test） ----------

/// 新增一項（空白不加）。回傳是否有加入
pub fn add(list: &mut Vec<Todo>, text: &str, id: String) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    list.push(Todo {
        id,
        text: text.to_string(),
        done: false,
        remind_at: None,
        notified: false,
        priority: None,
        quadrant: None,
        has_note: false,
    });
    true
}

pub fn toggle(list: &mut [Todo], id: &str) {
    if let Some(t) = list.iter_mut().find(|t| t.id == id) {
        t.done = !t.done;
    }
}

/// 改文字；改成空白就刪除
pub fn edit(list: &mut Vec<Todo>, id: &str, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return remove(list, id);
    }
    if let Some(t) = list.iter_mut().find(|t| t.id == id) {
        t.text = text.to_string();
    }
}

pub fn remove(list: &mut Vec<Todo>, id: &str) {
    list.retain(|t| t.id != id);
}

pub fn clear_done(list: &mut Vec<Todo>) {
    list.retain(|t| !t.done);
}

/// 拖曳排序
pub fn reorder(list: &mut Vec<Todo>, from: &str, to: &str, after: bool) {
    crate::videos::move_by_key(list, |t| t.id.as_str(), from, to, after);
}

/// 設定或清除提醒時間（重新設定後會再提醒一次）
pub fn set_remind(list: &mut [Todo], id: &str, at: Option<f64>) {
    if let Some(t) = list.iter_mut().find(|t| t.id == id) {
        t.remind_at = at;
        t.notified = false;
    }
}

/// 設定或清除優先順序（不認得的值當作清除）
pub fn set_priority(list: &mut [Todo], id: &str, p: Option<&str>) {
    if let Some(t) = list.iter_mut().find(|t| t.id == id) {
        t.priority = p.filter(|p| priority_info(p).is_some()).map(String::from);
    }
}

/// 放進某個象限（None = 未分類）
pub fn set_quadrant(list: &mut [Todo], id: &str, q: Option<u8>) {
    if let Some(t) = list.iter_mut().find(|t| t.id == id) {
        t.quadrant = q.filter(|q| quadrant_name(*q).is_some());
    }
}

/// 某個象限的項目（None = 未分類），未完成在前
pub fn in_quadrant(list: &[Todo], q: Option<u8>) -> Vec<&Todo> {
    ordered(list).into_iter().filter(|t| t.quadrant == q).collect()
}

/// 在某個象限新增一項，回傳是否有加入
pub fn add_to_quadrant(list: &mut Vec<Todo>, text: &str, id: String, q: u8) -> bool {
    let ok = add(list, text, id.clone());
    if ok {
        set_quadrant(list, &id, Some(q));
    }
    ok
}

/// 直接輸入的提醒時間 → (年, 月, 日, 時, 分)
///
/// 接受「2026/10/02 20:53」「2026-10-2 8:05」「10/2 20:53」（今年）「20:53」（今天）
/// 「2026年10月2日 20:53」、全形數字與冒號；只有日期時是 09:00。`today` 是 (年, 月, 日)。
pub fn parse_when(s: &str, today: (i32, u32, u32)) -> Option<(i32, u32, u32, u32, u32)> {
    let norm: String = s
        .trim()
        .chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
            '：' => ':',
            '－' | '-' | '.' | '年' | '月' | '／' => '/',
            '日' | 'T' | '　' => ' ',
            c => c,
        })
        .collect();
    let (mut date, mut time) = (None, None);
    for part in norm.split_whitespace() {
        if part.contains(':') && time.is_none() {
            time = Some(part);
        } else if part.contains('/') && date.is_none() {
            date = Some(part);
        } else {
            return None;
        }
    }
    if date.is_none() && time.is_none() {
        return None;
    }
    let num = |x: &str| x.parse::<u32>().ok();
    let (y, m, d) = match date {
        None => today,
        Some(dt) => {
            let p: Vec<&str> = dt.split('/').filter(|x| !x.is_empty()).collect();
            match p.as_slice() {
                [y, m, d] => (y.parse::<i32>().ok()?, num(m)?, num(d)?),
                [m, d] => (today.0, num(m)?, num(d)?),
                _ => return None,
            }
        }
    };
    let (h, mi) = match time {
        None => (9, 0),
        Some(t) => {
            let p: Vec<&str> = t.split(':').collect();
            match p.as_slice() {
                [h, mi] => (num(h)?, num(mi)?),
                _ => return None,
            }
        }
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    ((1000..=9999).contains(&y) && (1..=days).contains(&d) && h < 24 && mi < 60).then_some((y, m, d, h, mi))
}

/// 還沒提醒過、尚未完成的項目：(id, 提醒時間)
pub fn pending_reminders(list: &[Todo]) -> Vec<(String, f64)> {
    list.iter().filter(|t| !t.done && !t.notified).filter_map(|t| t.remind_at.map(|at| (t.id.clone(), at))).collect()
}

/// 時間到：標記為已提醒，回傳要顯示的項目（已完成、已提醒過或沒設時間就回傳 None）
pub fn take_due(list: &mut [Todo], id: &str) -> Option<Todo> {
    let t = list.iter_mut().find(|t| t.id == id && !t.done && !t.notified && t.remind_at.is_some())?;
    t.notified = true;
    Some(t.clone())
}

pub fn mark_done(list: &mut [Todo], id: &str) {
    if let Some(t) = list.iter_mut().find(|t| t.id == id) {
        t.done = true;
    }
}

/// 已過提醒時間、還沒完成
pub fn is_late(t: &Todo, now: f64) -> bool {
    !t.done && t.remind_at.is_some_and(|at| at <= now)
}

pub fn remaining(list: &[Todo]) -> usize {
    list.iter().filter(|t| !t.done).count()
}

/// 匯出 CSV（Excel 可直接開：開頭加 BOM、CRLF 換行）。`fmt_time` 把時間戳轉成文字
pub fn to_csv(list: &[Todo], fmt_time: impl Fn(f64) -> String) -> String {
    fn cell(s: &str) -> String {
        if s.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }
    let mut out = String::from("\u{feff}待辦事項,狀態,優先順序,任務矩陣,提醒時間\r\n");
    for t in ordered(list) {
        let row = [
            cell(&t.text),
            (if t.done { "已完成" } else { "未完成" }).to_string(),
            t.priority.as_deref().and_then(priority_info).map(|(_, _, n)| n).unwrap_or_default().to_string(),
            t.quadrant.and_then(quadrant_name).unwrap_or_default().to_string(),
            t.remind_at.map(&fmt_time).unwrap_or_default(),
        ];
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }
    out
}

/// 顯示順序：未完成在前、已完成在後，各自維持加入順序
pub fn ordered(list: &[Todo]) -> Vec<&Todo> {
    list.iter().filter(|t| !t.done).chain(list.iter().filter(|t| t.done)).collect()
}

// ---------- 提醒（在背景 service worker 執行，不碰 DOM） ----------

pub mod reminders {
    use super::*;
    use crate::chrome::{self, to_js};

    async fn load() -> Vec<Todo> {
        chrome::get_or(KEY, vec![]).await
    }

    async fn save(list: &[Todo]) {
        chrome::set(&[(KEY, to_js(list))]).await;
    }

    /// 依清單重設所有提醒鬧鐘（清單一改就呼叫）
    pub async fn sync_alarms() {
        for name in chrome::alarm_names().await {
            if name.starts_with(ALARM_PREFIX) {
                chrome::alarm_clear(&name).await;
            }
        }
        for (id, at) in pending_reminders(&load().await) {
            chrome::alarm_at(&format!("{ALARM_PREFIX}{id}"), at);
        }
    }

    /// 鬧鐘響了：跳出提醒小視窗
    pub async fn fire(id: &str) {
        let mut list = load().await;
        if take_due(&mut list, id).is_none() {
            return;
        }
        save(&list).await;
        chrome::open_popup_window(&format!("reminder.html?id={}", js_sys::encode_uri_component(id)), 440, 360).await;
    }

    /// 提醒視窗上的「完成」
    pub async fn done(id: &str) {
        let mut list = load().await;
        mark_done(&mut list, id);
        save(&list).await;
    }

    /// 提醒視窗上的「稍後提醒」
    pub async fn snooze(id: &str, minutes: f64) {
        let mut list = load().await;
        set_remind(&mut list, id, Some(chrome::now() + minutes * 60_000.0));
        save(&list).await;
    }

    pub async fn get(id: &str) -> Option<Todo> {
        load().await.into_iter().find(|t| t.id == id)
    }
}

// ---------- 畫面 ----------

pub(crate) mod view {
    use super::*;
    use crate::chrome::{self, to_js};
    use crate::ui::{doc, el, hide, listen, on_click, spawn};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{Element, HtmlButtonElement, HtmlElement, HtmlInputElement, KeyboardEvent};

    async fn load() -> Vec<Todo> {
        chrome::get_or(KEY, vec![]).await
    }

    async fn save(list: &[Todo]) {
        chrome::set(&[(KEY, to_js(list))]).await;
    }

    /// 讀出 → 修改 → 存回 → 重畫
    fn update(f: impl FnOnce(&mut Vec<Todo>) + 'static) {
        spawn(async move {
            let mut list = load().await;
            let before = list.clone();
            f(&mut list);
            save(&list).await;
            // 刪掉的項目，筆記和筆記裡的檔案也一起刪
            let mut gone: Vec<String> = vec![];
            for id in removed_ids(&before, &list) {
                if let Some(n) = chrome::get::<Note>(&note_key(&id)).await {
                    gone.extend(n.files.iter().map(|f| file_key(f)));
                }
                gone.push(note_key(&id));
            }
            if !gone.is_empty() {
                chrome::remove(&gone.iter().map(String::as_str).collect::<Vec<_>>()).await;
            }
            render_list(&list);
            render_matrix(&list);
        });
    }

    fn new_id() -> String {
        format!("{:x}{:04x}", chrome::now() as u64, (js_sys::Math::random() * 65536.0) as u32)
    }

    fn input(id: &str) -> HtmlInputElement {
        el(id).unchecked_into()
    }

    fn render_list(list: &[Todo]) {
        let n = remaining(list);
        el("todo-count").set_text_content(Some(&if n > 0 { format!("({n})") } else { String::new() }));
        let done = list.len() - n;
        el("todo-summary").set_text_content(Some(&if list.is_empty() {
            String::new()
        } else {
            format!("剩 {n} 項 · 已完成 {done} 項")
        }));
        el("todo-clear").unchecked_into::<HtmlButtonElement>().set_disabled(done == 0);
        el("todo-export").unchecked_into::<HtmlButtonElement>().set_disabled(list.is_empty());
        hide("todo-foot", list.is_empty());

        let ul = el("todo-list");
        ul.set_inner_html("");
        if list.is_empty() {
            ul.set_inner_html(r#"<li class="todo-none">還沒有待辦事項，在上面輸入後按 Enter</li>"#);
            return;
        }
        let q = crate::ui::search_text("todo-search");
        let shown: Vec<&Todo> = ordered(list).into_iter().filter(|t| crate::videos::matches(&q, &[&t.text])).collect();
        if shown.is_empty() {
            ul.set_inner_html(r#"<li class="todo-none">找不到符合的待辦事項</li>"#);
            return;
        }
        for t in shown {
            ul.append_child(&make_item(t)).unwrap();
        }
    }

    fn make_item(t: &Todo) -> Element {
        let li = doc().create_element("li").unwrap();
        li.set_class_name(if t.done { "todo-item done" } else { "todo-item" });
        li.set_inner_html(
            r#"<input type="checkbox"><div class="todo-body"><span class="todo-text" title="點兩下編輯" data-nt></span></div><span class="todo-prios" role="radiogroup" aria-label="優先順序"></span><button class="todo-note" type="button" title="筆記與圖片">📝</button><button class="todo-alarm" type="button" title="設定提醒時間">⏰</button><button class="bm-del" type="button" title="刪除">✕</button>"#,
        );
        // 優先順序：三個圓點直接排在 ⏰ 左邊，點一下選定，再點一次取消
        {
            // 結構：<span.todo-prios><span.prio-others>沒選的…</span>選中的</span>
            // 選中的固定緊貼 ⏰；沒選的浮在左邊、滑鼠移上去才出現（不佔位置，文字不會被擠）
            let box_ = li.query_selector(".todo-prios").unwrap().unwrap();
            let others = doc().create_element("span").unwrap();
            others.set_class_name("prio-others");
            box_.append_child(&others).unwrap();
            for (v, dot, name) in PRIORITIES {
                let on = t.priority.as_deref() == Some(v);
                let b = doc().create_element("button").unwrap();
                b.set_class_name(if on { "todo-prio on" } else { "todo-prio" });
                let _ = b.set_attribute("type", "button");
                let _ = b.set_attribute("role", "radio");
                let _ = b.set_attribute("aria-checked", if on { "true" } else { "false" });
                let _ = b.set_attribute("title", &if on { "再點一下取消".to_string() } else { format!("設為{name}") });
                let _ = b.set_attribute("aria-label", name);
                // 圓點和名稱分開：沒選的選項只顯示圓點，才不會蓋住待辦文字
                b.set_inner_html(r#"<span class="pd"></span><span class="pn"></span>"#);
                b.query_selector(".pd").unwrap().unwrap().set_text_content(Some(dot));
                b.query_selector(".pn").unwrap().unwrap().set_text_content(Some(name));
                let id = t.id.clone();
                listen(&b, "click", move |e| {
                    e.stop_propagation();
                    let id = id.clone();
                    update(move |l| set_priority(l, &id, if on { None } else { Some(v) }));
                });
                if on { box_.append_child(&b).unwrap() } else { others.append_child(&b).unwrap() };
            }
            if let Some(v) = t.priority.as_deref().filter(|p| priority_info(p).is_some()) {
                let _ = li.class_list().add_1(&format!("prio-{v}"));
                let _ = li.class_list().add_1("has-prio");
            }
        }
        if let Some(name) = t.quadrant.and_then(quadrant_name) {
            let tag = doc().create_element("span").unwrap();
            tag.set_class_name(&format!("todo-quad q{}", t.quadrant.unwrap_or(0)));
            tag.set_text_content(Some(&format!("◆ {name}")));
            let _ = tag.set_attribute("title", "任務矩陣分類（點標題列的「任務矩陣」調整）");
            li.query_selector(".todo-body").unwrap().unwrap().append_child(&tag).unwrap();
        }
        if let Some(at) = t.remind_at {
            // 提醒時間標籤，點它也能改時間
            let chip = doc().create_element("button").unwrap();
            let late = is_late(t, chrome::now());
            chip.set_class_name(if late { "todo-when late" } else { "todo-when" });
            let _ = chip.set_attribute("type", "button");
            let _ = chip.set_attribute("title", "修改提醒時間");
            chip.set_text_content(Some(&format!(
                "⏰ {}{}",
                when_label(at),
                if late { " · 已到期" } else { "" }
            )));
            li.query_selector(".todo-body").unwrap().unwrap().append_child(&chip).unwrap();
            let (li2, id, at2) = (li.clone(), t.id.clone(), t.remind_at);
            listen(&chip, "click", move |_| toggle_picker(&li2, id.clone(), at2));
            let _ = li.class_list().add_1("has-alarm");
        }
        {
            let (li2, id, at) = (li.clone(), t.id.clone(), t.remind_at);
            listen(&li.query_selector(".todo-alarm").unwrap().unwrap(), "click", move |_| {
                toggle_picker(&li2, id.clone(), at)
            });
        }
        {
            let nb = li.query_selector(".todo-note").unwrap().unwrap();
            if t.has_note {
                let _ = nb.class_list().add_1("has");
                let _ = nb.set_attribute("title", "打開筆記（有內容）");
            }
            let id = t.id.clone();
            listen(&nb, "click", move |e| {
                e.stop_propagation();
                crate::todo_note::open(id.clone());
            });
        }
        li.prepend_with_node_1(&crate::drag::grip()).unwrap();
        crate::drag::sortable(
            crate::drag::Sortable {
                item: li.clone(),
                handle: Some(li.clone()),
                zone: li.clone(),
                group: "todo",
                id: t.id.clone(),
                can_contain: false,
            },
            std::rc::Rc::new(|from, to, pos| {
                update(move |l| reorder(l, &from, &to, pos == crate::drag::Pos::After));
            }),
        );
        let cb: HtmlInputElement = li.query_selector("input").unwrap().unwrap().unchecked_into();
        cb.set_checked(t.done);
        let _ = cb.set_attribute("aria-label", &format!("完成：{}", t.text));
        let span = li.query_selector(".todo-text").unwrap().unwrap();
        span.set_text_content(Some(&t.text));

        let id = t.id.clone();
        listen(&cb, "change", move |_| {
            let id = id.clone();
            update(move |l| toggle(l, &id));
        });
        let id = t.id.clone();
        listen(&li.query_selector(".bm-del").unwrap().unwrap(), "click", move |_| {
            let id = id.clone();
            update(move |l| remove(l, &id));
        });
        let (id, text, span2) = (t.id.clone(), t.text.clone(), span.clone());
        listen(&span, "dblclick", move |_| start_edit(&span2, id.clone(), text.clone()));
        li
    }

    // ---------- 任務矩陣 ----------

    fn matrix_open() -> bool {
        el("quad").unchecked_into::<web_sys::HtmlDialogElement>().open()
    }

    /// 象限裡的一項：勾選完成、文字、✕ 移回未分類；可拖曳
    fn matrix_item(t: &Todo) -> Element {
        let li = doc().create_element("li").unwrap();
        li.set_class_name(if t.done { "qd-item done" } else { "qd-item" });
        li.set_inner_html(r#"<input type="checkbox"><span class="qd-text" data-nt></span><button class="bm-del" type="button">✕</button>"#);
        let cb: HtmlInputElement = li.query_selector("input").unwrap().unwrap().unchecked_into();
        cb.set_checked(t.done);
        let _ = cb.set_attribute("aria-label", &format!("完成：{}", t.text));
        li.query_selector(".qd-text").unwrap().unwrap().set_text_content(Some(&t.text));
        if let Some((_, dot, name)) = t.priority.as_deref().and_then(priority_info) {
            let p = doc().create_element("span").unwrap();
            p.set_class_name("qd-prio");
            p.set_text_content(Some(dot));
            let _ = p.set_attribute("title", name);
            li.insert_before(&p, li.query_selector(".qd-text").ok().flatten().as_ref().map(|e| e.unchecked_ref::<web_sys::Node>())).unwrap();
        }
        let id = t.id.clone();
        listen(&cb, "change", move |_| {
            let id = id.clone();
            update(move |l| toggle(l, &id));
        });
        let x = li.query_selector(".bm-del").unwrap().unwrap();
        let in_q = t.quadrant.is_some();
        let _ = x.set_attribute("title", if in_q { "移回未分類" } else { "刪除這項待辦" });
        let id = t.id.clone();
        listen(&x, "click", move |e| {
            e.stop_propagation();
            let id = id.clone();
            if in_q {
                update(move |l| set_quadrant(l, &id, None));
            } else {
                update(move |l| remove(l, &id));
            }
        });
        // 拖到另一項上 = 放進那一項的象限、排在它前後
        let target_q = t.quadrant;
        crate::drag::sortable(
            crate::drag::Sortable { item: li.clone(), handle: Some(li.clone()), zone: li.clone(), group: "quad", id: t.id.clone(), can_contain: false },
            std::rc::Rc::new(move |from, to, pos| {
                update(move |l| {
                    set_quadrant(l, &from, target_q);
                    reorder(l, &from, &to, pos == crate::drag::Pos::After);
                })
            }),
        );
        li
    }

    fn fill_list(ul: &Element, items: &[&Todo], empty: &str) {
        ul.set_inner_html("");
        if items.is_empty() {
            let li = doc().create_element("li").unwrap();
            li.set_class_name("qd-empty");
            li.set_text_content(Some(empty));
            ul.append_child(&li).unwrap();
        }
        for t in items {
            ul.append_child(&matrix_item(t)).unwrap();
        }
    }

    fn render_matrix(list: &[Todo]) {
        if !matrix_open() {
            return;
        }
        // 未分類：只列未完成的
        let unsorted: Vec<&Todo> = in_quadrant(list, None).into_iter().filter(|t| !t.done).collect();
        el("qd-unsorted-count").set_text_content(Some(&format!("（{}）", unsorted.len())));
        fill_list(&el("qd-unsorted"), &unsorted, "所有待辦都分類好了 👍");
        for (q, _, _) in QUADRANTS {
            let items = in_quadrant(list, Some(q));
            let left = items.iter().filter(|t| !t.done).count();
            el(&format!("qd-count-{q}")).set_text_content(Some(&if left > 0 { left.to_string() } else { String::new() }));
            fill_list(&el(&format!("qd-list-{q}")), &items, "拖曳待辦到這裡，或在下方新增");
        }
    }

    fn open_matrix() {
        let d: web_sys::HtmlDialogElement = el("quad").unchecked_into();
        if !d.open() {
            let _ = d.show_modal();
        }
        spawn(async { render_matrix(&load().await) });
    }

    fn setup_matrix() {
        on_click("todo-quad-btn", open_matrix);
        on_click("qd-close", || el("quad").unchecked_into::<web_sys::HtmlDialogElement>().close());
        listen(&el("quad"), "click", |e| {
            if e.target().map(|t| t == el("quad").into()).unwrap_or(false) {
                el("quad").unchecked_into::<web_sys::HtmlDialogElement>().close();
            }
        });
        // 象限本身與「未分類」都可以放
        let mut zones: Vec<(String, Option<u8>)> = QUADRANTS.iter().map(|(q, _, _)| (format!("qd-box-{q}"), Some(*q))).collect();
        zones.push(("qd-unsorted-box".into(), None));
        for (zone_id, q) in zones {
            let zone = el(&zone_id).unchecked_into::<Element>();
            crate::drag::sortable(
                crate::drag::Sortable { item: zone.clone(), handle: None, zone, group: "quad", id: zone_id.clone(), can_contain: true },
                std::rc::Rc::new(move |from, _to, _pos| update(move |l| set_quadrant(l, &from, q))),
            );
        }
        // 各象限的新增欄
        for (q, _, _) in QUADRANTS {
            let input_id = format!("qd-add-{q}");
            let inp: HtmlInputElement = el(&input_id).unchecked_into();
            let i2 = inp.clone();
            listen(&inp, "keydown", move |e| {
                let k: &KeyboardEvent = e.unchecked_ref();
                if k.key() == "Enter" && !k.is_composing() {
                    e.prevent_default();
                    let v = i2.value();
                    if v.trim().is_empty() {
                        return;
                    }
                    i2.set_value("");
                    update(move |l| {
                        add_to_quadrant(l, &v, new_id(), q);
                    });
                }
            });
        }
    }

    // ---------- 提醒時間 ----------

    fn date_of(ms: f64) -> js_sys::Date {
        js_sys::Date::new(&JsValue::from_f64(ms))
    }

    /// 本地時間的「年月日」序號，用來判斷今天／明天
    fn day_key(d: &js_sys::Date) -> (u32, u32, u32) {
        (d.get_full_year(), d.get_month(), d.get_date())
    }

    /// 「今天 14:30」「明天 09:00」「10/5 (週日) 14:30」
    pub(crate) fn when_label(ms: f64) -> String {
        let d = date_of(ms);
        let now = chrome::now();
        let hm = format!("{:02}:{:02}", d.get_hours(), d.get_minutes());
        let day = if day_key(&d) == day_key(&date_of(now)) {
            "今天".to_string()
        } else if day_key(&d) == day_key(&date_of(now + 86_400_000.0)) {
            "明天".to_string()
        } else if day_key(&d) == day_key(&date_of(now - 86_400_000.0)) {
            "昨天".to_string()
        } else {
            let wd = ["日", "一", "二", "三", "四", "五", "六"][d.get_day() as usize];
            let year = if d.get_full_year() != date_of(now).get_full_year() { format!("{}/", d.get_full_year()) } else { String::new() };
            format!("{year}{}/{} (週{wd})", d.get_month() + 1, d.get_date())
        };
        format!("{day} {hm}")
    }

    /// <input type="datetime-local"> 的值（本地時間 YYYY-MM-DDTHH:MM）
    /// 輸入框顯示的格式：2026/10/02 20:53（24 小時制）
    fn to_text_input(ms: f64) -> String {
        let d = date_of(ms);
        format!("{:04}/{:02}/{:02} {:02}:{:02}", d.get_full_year(), d.get_month() + 1, d.get_date(), d.get_hours(), d.get_minutes())
    }

    fn from_text_input(v: &str) -> Option<f64> {
        let now = date_of(chrome::now());
        let (y, m, d, h, mi) = parse_when(v, (now.get_full_year() as i32, now.get_month() + 1, now.get_date()))?;
        Some(js_sys::Date::new_with_year_month_day_hr_min_sec(y as u32, m as i32 - 1, d as i32, h as i32, mi as i32, 0).get_time())
    }

    fn to_local_input(ms: f64) -> String {
        let d = date_of(ms);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}",
            d.get_full_year(),
            d.get_month() + 1,
            d.get_date(),
            d.get_hours(),
            d.get_minutes()
        )
    }

    fn from_local_input(v: &str) -> Option<f64> {
        if v.is_empty() {
            return None;
        }
        // 沒有時區的日期時間字串，瀏覽器會當成本地時間
        let t = js_sys::Date::new(&JsValue::from_str(v)).get_time();
        (!t.is_nan()).then_some(t)
    }

    /// 某天幾點（day_offset：0 今天、1 明天）
    fn at_hour(day_offset: f64, hour: u32) -> f64 {
        let d = date_of(chrome::now() + day_offset * 86_400_000.0);
        d.set_hours(hour);
        d.set_minutes(0);
        d.set_seconds(0);
        d.set_milliseconds(0);
        d.get_time()
    }

    /// 在項目下方展開／收起時間選擇器
    fn toggle_picker(li: &Element, id: String, current: Option<f64>) {
        if let Some(old) = li.query_selector(".todo-picker").ok().flatten() {
            old.remove();
            return;
        }
        // 一次只開一個
        if let Ok(others) = doc().query_selector_all("#todo-list .todo-picker") {
            for i in 0..others.length() {
                if let Some(n) = others.item(i) {
                    n.unchecked_into::<Element>().remove();
                }
            }
        }
        let now = chrome::now();
        let quick: [(&str, f64); 5] = [
            ("5 分鐘後", now + 300_000.0),
            ("10 分鐘後", now + 600_000.0),
            ("30 分鐘後", now + 1_800_000.0),
            ("明天 09:00", at_hour(1.0, 9)),
            ("明天 18:00", at_hour(1.0, 18)),
        ];

        let p = doc().create_element("div").unwrap();
        p.set_class_name("todo-picker");
        p.set_inner_html(
            r#"<div class="todo-quick"></div><div class="todo-pick-row"><div class="todo-dt-box"><input type="text" class="todo-dt" inputmode="numeric" autocomplete="off" spellcheck="false" placeholder="2026/10/02 20:30" aria-label="提醒時間（可直接輸入，例如 2026/10/02 20:30）"><span class="todo-cal" title="從月曆選擇">📅<input type="datetime-local" class="todo-cal-input" tabindex="-1" aria-hidden="true"></span></div><button class="btn primary todo-set" type="button">設定</button><button class="btn todo-unset" type="button">取消提醒</button></div><div class="todo-pick-msg"></div>"#,
        );
        let q = p.query_selector(".todo-quick").unwrap().unwrap();
        for (label, at) in quick {
            let b = doc().create_element("button").unwrap();
            b.set_class_name("chip");
            let _ = b.set_attribute("type", "button");
            b.set_text_content(Some(label));
            let id = id.clone();
            listen(&b, "click", move |_| {
                let id = id.clone();
                update(move |l| set_remind(l, &id, Some(at)));
            });
            q.append_child(&b).unwrap();
        }
        let dt: HtmlInputElement = p.query_selector(".todo-dt").unwrap().unwrap().unchecked_into();
        dt.set_value(&to_text_input(current.unwrap_or(now + 3_600_000.0)));
        // 月曆：選好後填回輸入框
        let cal: HtmlInputElement = p.query_selector(".todo-cal-input").unwrap().unwrap().unchecked_into();
        cal.set_min(&to_local_input(now));
        {
            let (cal2, dt) = (cal.clone(), dt.clone());
            listen(&cal, "focus", {
                let (cal, dt) = (cal2.clone(), dt.clone());
                move |_| {
                    if let Some(at) = from_text_input(&dt.value()) {
                        cal.set_value(&to_local_input(at));
                    }
                }
            });
            listen(&cal, "change", move |_| {
                if let Some(at) = from_local_input(&cal2.value()) {
                    dt.set_value(&to_text_input(at));
                }
            });
        }
        let unset = p.query_selector(".todo-unset").unwrap().unwrap();
        unset.unchecked_ref::<HtmlElement>().set_hidden(current.is_none());

        let set = {
            let (id, dt, p2) = (id.clone(), dt.clone(), p.clone());
            move || match from_text_input(&dt.value()) {
                Some(at) if at > chrome::now() => {
                    let id = id.clone();
                    update(move |l| set_remind(l, &id, Some(at)));
                }
                Some(_) => text_in(&p2, ".todo-pick-msg", "請輸入未來的時間"),
                None => text_in(&p2, ".todo-pick-msg", "看不懂這個時間，請照「2026/10/02 20:30」的格式輸入"),
            }
        };
        let set = std::rc::Rc::new(set);
        {
            let set = set.clone();
            listen(&p.query_selector(".todo-set").unwrap().unwrap(), "click", move |_| set());
        }
        listen(&dt, "keydown", move |e| {
            let k: &KeyboardEvent = e.unchecked_ref();
            if k.key() == "Enter" {
                e.prevent_default();
                set();
            }
        });
        listen(&unset, "click", move |_| {
            let id = id.clone();
            update(move |l| set_remind(l, &id, None));
        });
        // 選擇器裡的操作不要觸發拖曳
        let _ = li.set_attribute("draggable", "false");
        li.append_child(&p).unwrap();
        let _ = dt.unchecked_ref::<HtmlElement>().focus();
    }

    fn text_in(parent: &Element, sel: &str, s: &str) {
        if let Some(e) = parent.query_selector(sel).ok().flatten() {
            e.set_text_content(Some(s));
        }
    }

    /// 點兩下文字：原地改成輸入框；Enter 或離開儲存、Esc 取消
    fn start_edit(span: &Element, id: String, text: String) {
        let inp: HtmlInputElement = doc().create_element("input").unwrap().unchecked_into();
        inp.set_class_name("todo-edit");
        inp.set_value(&text);
        // 編輯時暫停拖曳，才能用滑鼠選取文字
        if let Some(li) = span.closest("li").ok().flatten() {
            let _ = li.set_attribute("draggable", "false");
        }
        let _ = span.replace_with_with_node_1(&inp);
        let _ = inp.focus();
        inp.select();

        let finished = std::rc::Rc::new(std::cell::Cell::new(false));
        let (f1, i1, id1) = (finished.clone(), inp.clone(), id.clone());
        listen(&inp, "keydown", move |e| {
            let k: &KeyboardEvent = e.unchecked_ref();
            if k.is_composing() {
                return;
            }
            match k.key().as_str() {
                "Enter" => {
                    f1.set(true);
                    let (id, v) = (id1.clone(), i1.value());
                    update(move |l| edit(l, &id, &v));
                }
                "Escape" => {
                    e.stop_propagation(); // 不要順便關掉整個清單
                    f1.set(true);
                    spawn(async { render_list(&load().await) });
                }
                _ => {}
            }
        });
        let i2 = inp.clone();
        listen(&inp, "blur", move |_| {
            if !finished.get() {
                finished.set(true);
                let (id, v) = (id.clone(), i2.value());
                update(move |l| edit(l, &id, &v));
            }
        });
    }

    fn is_open() -> bool {
        !el("todo").hidden()
    }

    /// 被別的面板打開時關掉自己
    pub fn close() {
        if is_open() {
            set_open(false);
        }
    }

    fn set_open(open: bool) {
        if open {
            crate::ui::close_panels_except("todo");
        }
        hide("todo", !open);
        let _ = el("todo-toggle").class_list().toggle_with_force("on", open);
        let _ = el("todo-toggle").set_attribute("aria-expanded", if open { "true" } else { "false" });
        if open {
            let _ = el("todo-input").focus();
            crate::weather::refresh();
            // 重畫一次，讓「已到期」標示是最新的
            spawn(async { render_list(&load().await) });
        }
    }

    fn add_from_input() {
        let v = input("todo-input").value();
        if v.trim().is_empty() {
            return;
        }
        input("todo-input").set_value("");
        update(move |l| {
            add(l, &v, new_id());
        });
    }

    pub fn start() {
        on_click("todo-toggle", || set_open(!is_open()));
        on_click("todo-close", || set_open(false));
        on_click("todo-add", add_from_input);
        on_click("todo-clear", || update(clear_done));
        on_click("todo-export", || {
            spawn(async {
                let list = load().await;
                let csv = to_csv(&list, |ms| to_local_input(ms).replace('T', " "));
                let name = format!("TODO-{}.csv", to_local_input(chrome::now()).split('T').next().unwrap_or("").replace('-', ""));
                crate::ui::download_text(&name, &csv, "text/csv;charset=utf-8");
            })
        });
        crate::weather::start();
        crate::timer::start();
        setup_matrix();
        crate::ui::search_box("todo-search", || spawn(async { render_list(&load().await) }));
        listen(&el("todo-input"), "keydown", |e| {
            let k: &KeyboardEvent = e.unchecked_ref();
            if k.key() == "Enter" && !k.is_composing() {
                e.prevent_default();
                add_from_input();
            }
        });

        // 點清單外面或按 Esc 關閉
        let d: web_sys::EventTarget = doc().into();
        listen(&d, "mousedown", |e| {
            if !is_open() {
                return;
            }
            let inside = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                .map(|n| {
                    el("todo").contains(Some(&n))
                        || el("todo-toggle").contains(Some(&n))
                        || el("quad").contains(Some(&n))
                        || el("note-dialog").contains(Some(&n))
                })
                .unwrap_or(false);
            if !inside {
                set_open(false);
            }
        });
        listen(&d, "keydown", |e| {
            if is_open() && !matrix_open() && !crate::todo_note::is_open() && e.unchecked_ref::<KeyboardEvent>().key() == "Escape" {
                set_open(false);
                let _ = el("todo-toggle").focus();
            }
        });

        // 其他分頁改了清單時同步更新（正在編輯時不打斷）
        let cb = Closure::<dyn FnMut(JsValue, JsValue)>::new(|changes: JsValue, _area: JsValue| {
            if js_sys::Reflect::has(&changes, &KEY.into()).unwrap_or(false)
                && doc().query_selector("#todo-list .todo-edit, #todo-list .todo-picker").ok().flatten().is_none()
            {
                spawn(async {
                    let l = load().await;
                    render_list(&l);
                    render_matrix(&l);
                });
            }
        });
        chrome::on_storage_changed(&cb);
        cb.forget();

        spawn(async { render_list(&load().await) });

        // 從提醒通知點進來（newtab.html#todo）：直接打開清單
        let loc = web_sys::window().unwrap().location();
        if loc.hash().unwrap_or_default() == "#todo" {
            let _ = web_sys::window().unwrap().history().and_then(|h| h.replace_state_with_url(&JsValue::NULL, "", Some("newtab.html")));
            set_open(true);
        } else {
            // 每開一個新分頁都先打開 TODO（其他面板關閉）；游標留在 Google 搜尋框，照樣可以直接打字搜尋
            set_open(true);
            if let Some(q) = doc().get_element_by_id("q") {
                let _ = q.unchecked_into::<HtmlElement>().focus();
            }
        }
    }
}

pub use view::{close, start};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_files() {
        let chip = file_chip("ab12", "報告<1>.pdf", 1_300_000.0, "application/pdf");
        assert!(chip.contains(r#"data-fid="ab12""#) && chip.contains("報告&lt;1&gt;.pdf") && chip.contains("1.2 MB") && chip.contains("📄"));
        assert_eq!(fids_in(&format!("x{chip}y{}{chip}", file_chip("cd34", "b.zip", 10.0, ""))), vec!["ab12", "cd34"]);
        assert_eq!(file_icon("a.ZIP", "application/zip"), "🗜");
        assert_eq!(file_icon("a.docx", ""), "📝");
        assert_eq!(file_icon("a.bin", ""), "📎");
        assert!(viewable("a.pdf", "") && viewable("a.mp4", "video/mp4") && !viewable("a.docx", "application/msword"));
        assert!(inline_image("image/png") && !inline_image("image/heic"));
        assert_eq!(size_label(10.0), "1 KB");
        let n = Note { files: vec!["ab12".into()], ..Default::default() };
        assert!(!n.is_empty());
    }

    #[test]
    fn note_html_from_old_format() {
        let old = Note { text: "a<b\n第二行".into(), images: vec!["data:image/png;base64,AAA".into()], ..Default::default() };
        assert_eq!(old.to_html(), "a&lt;b<br>第二行<img src=\"data:image/png;base64,AAA\">");
        assert!(!old.is_empty());
        let new = Note { html: "<img src=\"data:x\">".into(), ..Default::default() };
        assert!(!new.is_empty());
        assert!(Note::default().is_empty());
    }

    fn ids(v: &[&Todo]) -> Vec<String> {
        v.iter().map(|t| t.id.clone()).collect()
    }

    #[test]
    fn add_toggle_edit_remove() {
        let mut l = vec![];
        assert!(add(&mut l, "  買牛奶 ", "a".into()));
        assert!(l[0].remind_at.is_none());
        assert!(!add(&mut l, "   ", "x".into()));
        add(&mut l, "寫論文", "b".into());
        add(&mut l, "回信", "c".into());
        assert_eq!(l[0].text, "買牛奶");
        assert_eq!(remaining(&l), 3);

        toggle(&mut l, "a");
        assert_eq!(remaining(&l), 2);
        assert_eq!(ids(&ordered(&l)), ["b", "c", "a"]);

        edit(&mut l, "b", " 寫論文第三章 ");
        assert_eq!(l[1].text, "寫論文第三章");
        edit(&mut l, "c", "  ");
        assert_eq!(l.len(), 2);

        clear_done(&mut l);
        assert_eq!(ids(&ordered(&l)), ["b"]);
        remove(&mut l, "b");
        assert!(l.is_empty());
    }

    #[test]
    fn quadrants() {
        let mut l = vec![];
        add(&mut l, "A", "a".into());
        add(&mut l, "B", "b".into());
        assert!(add_to_quadrant(&mut l, "論文", "c".into(), 2));
        assert!(!add_to_quadrant(&mut l, "  ", "x".into(), 2));
        set_quadrant(&mut l, "a", Some(1));
        set_quadrant(&mut l, "b", Some(9)); // 不存在的象限 = 未分類
        let ids = |v: Vec<&Todo>| v.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(in_quadrant(&l, Some(1))), ["a"]);
        assert_eq!(ids(in_quadrant(&l, Some(2))), ["c"]);
        assert_eq!(ids(in_quadrant(&l, None)), ["b"]);
        set_quadrant(&mut l, "a", None);
        assert_eq!(ids(in_quadrant(&l, None)), ["a", "b"]);
        assert_eq!(quadrant_name(3), Some("緊急不重要"));
    }

    #[test]
    fn reminders_flow() {
        let mut l = vec![];
        add(&mut l, "開會", "a".into());
        add(&mut l, "繳費", "b".into());
        add(&mut l, "看書", "c".into());
        set_remind(&mut l, "a", Some(1000.0));
        set_remind(&mut l, "b", Some(2000.0));
        toggle(&mut l, "b"); // 已完成的不提醒
        assert_eq!(pending_reminders(&l), vec![("a".to_string(), 1000.0)]);
        assert!(is_late(&l[0], 1000.0) && !is_late(&l[0], 999.0));
        assert!(!is_late(&l[1], 5000.0) && !is_late(&l[2], 5000.0));

        // 時間到只提醒一次
        assert_eq!(take_due(&mut l, "a").map(|t| t.text), Some("開會".to_string()));
        assert!(take_due(&mut l, "a").is_none());
        assert!(take_due(&mut l, "c").is_none());
        assert!(pending_reminders(&l).is_empty());

        // 重新設定（稍後提醒）會再提醒
        set_remind(&mut l, "a", Some(3000.0));
        assert_eq!(pending_reminders(&l), vec![("a".to_string(), 3000.0)]);
        mark_done(&mut l, "a");
        assert!(pending_reminders(&l).is_empty());
        set_remind(&mut l, "c", None);
        assert!(l[2].remind_at.is_none());

        // 直接輸入提醒時間
        let today = (2026, 10, 2);
        assert_eq!(parse_when("2026/10/02 20:53", today), Some((2026, 10, 2, 20, 53)));
        assert_eq!(parse_when(" 2026-10-3 8:05 ", today), Some((2026, 10, 3, 8, 5)));
        assert_eq!(parse_when("10/5 18:00", today), Some((2026, 10, 5, 18, 0)));
        assert_eq!(parse_when("21:30", today), Some((2026, 10, 2, 21, 30)));
        assert_eq!(parse_when("2026/10/04", today), Some((2026, 10, 4, 9, 0)));
        assert_eq!(parse_when("2026年10月6日 07:15", today), Some((2026, 10, 6, 7, 15)));
        assert_eq!(parse_when("２０２６／１０／０７　２０：００", today), Some((2026, 10, 7, 20, 0)));
        assert_eq!(parse_when("2026-10-08T09:30", today), Some((2026, 10, 8, 9, 30)));
        assert_eq!(parse_when("2028/2/29 10:00", today), Some((2028, 2, 29, 10, 0)));
        for bad in ["", "abc", "2026/13/01 10:00", "2026/02/30 10:00", "2027/2/29 1:00", "24:00", "10:60", "2026/10/02 20:53 x", "1/2/3/4 10:00"] {
            assert_eq!(parse_when(bad, today), None, "{bad}");
        }

        // 匯出 CSV：逗號、引號、換行要跳脫；未完成排前面
        let mut c = vec![];
        add(&mut c, "買牛奶, 蛋", "1".into());
        add(&mut c, "說\"嗨\"", "2".into());
        toggle(&mut c, "1");
        set_remind(&mut c, "2", Some(5.0));
        set_priority(&mut c, "2", Some("high"));
        set_priority(&mut c, "1", Some("bogus"));
        set_quadrant(&mut c, "2", Some(1));
        assert!(c[0].priority.is_none());
        let csv = to_csv(&c, |ms| format!("T{ms}"));
        assert_eq!(
            csv,
            "\u{feff}待辦事項,狀態,優先順序,任務矩陣,提醒時間\r\n\"說\"\"嗨\"\"\",未完成,高優先,重要且緊急,T5\r\n\"買牛奶, 蛋\",已完成,,,\r\n"
        );

        // 舊資料（沒有提醒欄位）讀得進來，沒設提醒時存檔也不多欄位
        let old: Todo = serde_json::from_str(r#"{"id":"x","text":"t","done":false}"#).unwrap();
        assert_eq!(serde_json::to_string(&old).unwrap(), r#"{"id":"x","text":"t","done":false}"#);
    }
}
