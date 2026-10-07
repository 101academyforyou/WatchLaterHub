//! TODO 的 📝 筆記小視窗：改主題、寫更多筆記，圖片直接貼在筆記裡（文字和圖片混排）
//!
//! 筆記存在 `todoNote:<id>`（chrome.storage），清單只記 `hasNote`。
//! 打字停 0.4 秒自動存；圖片超過 1600px 或太大時縮小成 JPEG。
//! 貼上時只收純文字和圖片，不收其他網頁的格式，所以存下來的 HTML 只有文字、換行和 <img>。

use crate::chrome::{self, to_js};
use crate::todo::{
    file_chip, file_key, fids_in, inline_image, note_key, set_has_note, set_title, viewable, Note, NoteFile, Todo, KEY, MAX_FILE_BYTES,
};
use crate::ui::{doc, el, listen, on_click, spawn, timeout};
use std::cell::{Cell, RefCell};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    ClipboardEvent, DragEvent, File, FileReader, HtmlCanvasElement, HtmlDialogElement, HtmlDocument, HtmlElement,
    HtmlImageElement, HtmlInputElement, KeyboardEvent, Range,
};

const MAX_SIDE: f64 = 1600.0;
const MAX_BYTES: usize = 1_500_000;

thread_local! {
    /// 正在編輯的待辦 id
    static CURRENT: RefCell<Option<String>> = const { RefCell::new(None) };
    static SAVE_SEQ: Cell<u32> = const { Cell::new(0) };
    /// 筆記區最後的游標位置（按「上傳圖片」時筆記區會失去焦點）
    static CARET: RefCell<Option<Range>> = const { RefCell::new(None) };
    /// 選取中的圖片（顯示大小工具列和拖曳點）
    static SELECTED: RefCell<Option<HtmlImageElement>> = const { RefCell::new(None) };
    /// 拖曳調整大小：(起點 x, 起始寬度)
    static RESIZING: Cell<Option<(f64, f64)>> = const { Cell::new(None) };
    /// 這次打開後出現過的檔案 id（關閉時刪掉已經不在筆記裡的）
    static SESSION_FILES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

// ---------- 圖片大小 ----------

fn selected() -> Option<HtmlImageElement> {
    SELECTED.with(|s| s.borrow().clone()).filter(|img| editor().contains(Some(img)))
}

fn deselect() {
    if let Some(img) = SELECTED.with(|s| s.borrow_mut().take()) {
        let _ = img.class_list().remove_1("sel");
    }
    el("note-imgbar").set_hidden(true);
    el("note-handle").set_hidden(true);
}

fn select(img: HtmlImageElement) {
    deselect();
    let _ = img.class_list().add_1("sel");
    SELECTED.with(|s| *s.borrow_mut() = Some(img));
    el("note-imgbar").set_hidden(false);
    el("note-handle").set_hidden(false);
    place_tools();
}

/// 工具列放在圖片上方、拖曳點放在右下角（座標相對於視窗本身）
fn place_tools() {
    let Some(img) = selected() else { return deselect() };
    let d = dialog();
    let dr = d.get_bounding_client_rect();
    let r = img.get_bounding_client_rect();
    let er = editor().get_bounding_client_rect();
    // 圖片捲到筆記區外面就先藏起來
    let visible = r.bottom() > er.top() && r.top() < er.bottom();
    el("note-imgbar").set_hidden(!visible);
    el("note-handle").set_hidden(!visible || r.bottom() > er.bottom());
    let (ox, oy) = (d.scroll_left() as f64 - dr.left() - d.client_left() as f64, d.scroll_top() as f64 - dr.top() - d.client_top() as f64);
    let bar = el("note-imgbar");
    // 優先放在圖片上方（不蓋住圖片）；上方沒空間才放進圖片裡
    let bh = bar.offset_height() as f64;
    let above = r.top() - bh - 4.0;
    let top = (if above >= er.top() { above } else { r.top().max(er.top()) + 6.0 } + oy).max(0.0);
    let left = (r.left() + 6.0 + ox).max(0.0);
    let _ = bar.style().set_property("top", &format!("{top}px"));
    let _ = bar.style().set_property("left", &format!("{left}px"));
    let h = el("note-handle");
    let _ = h.style().set_property("top", &format!("{}px", r.bottom() - 9.0 + oy));
    let _ = h.style().set_property("left", &format!("{}px", r.right() - 9.0 + ox));
    // 標示目前的大小
    let cur = img.style().get_property_value("width").unwrap_or_default();
    for b in bar.query_selector_all("button[data-w]").map(|l| (0..l.length()).filter_map(|i| l.item(i)).collect::<Vec<_>>()).unwrap_or_default() {
        let b: web_sys::Element = b.unchecked_into();
        let w = b.get_attribute("data-w").unwrap_or_default();
        let on = if w == "0" { cur.is_empty() } else { cur == format!("{w}%") };
        let _ = b.class_list().toggle_with_force("on", on);
    }
}

/// 筆記區可放內容的寬度（扣掉左右留白）
fn content_width() -> f64 {
    let e = editor();
    let cs = web_sys::window().unwrap().get_computed_style(&e).ok().flatten();
    let pad = |p: &str| cs.as_ref().and_then(|c| c.get_property_value(p).ok()).and_then(|v| v.trim_end_matches("px").parse::<f64>().ok()).unwrap_or(0.0);
    (e.client_width() as f64 - pad("padding-left") - pad("padding-right")).max(1.0)
}

/// 設定圖片寬度（佔筆記區的百分比；0 = 原始大小）
fn set_width(img: &HtmlImageElement, pct: f64) {
    if pct <= 0.0 {
        let _ = img.style().remove_property("width");
        if img.get_attribute("style").is_some_and(|s| s.trim().is_empty()) {
            let _ = img.remove_attribute("style");
        }
    } else {
        let _ = img.style().set_property("width", &format!("{}%", pct.clamp(5.0, 100.0).round()));
    }
}

fn dialog() -> HtmlDialogElement {
    el("note-dialog").unchecked_into()
}

fn title_input() -> HtmlInputElement {
    el("note-title").unchecked_into()
}

fn editor() -> HtmlElement {
    el("note-text")
}

pub fn is_open() -> bool {
    dialog().open()
}

fn msg(s: &str) {
    el("note-msg").set_text_content(Some(s));
}

/// 打開某個待辦事項的筆記
pub fn open(id: String) {
    spawn(async move {
        let list: Vec<Todo> = chrome::get_or(KEY, vec![]).await;
        let Some(t) = list.iter().find(|t| t.id == id) else { return };
        let note: Note = chrome::get_or(&note_key(&id), Note::default()).await;
        title_input().set_value(&t.text);
        editor().set_inner_html(&note.to_html());
        SESSION_FILES.with(|s| *s.borrow_mut() = note.files.clone());
        CURRENT.with(|c| *c.borrow_mut() = Some(id));
        CARET.with(|c| *c.borrow_mut() = None);
        deselect();
        msg("");
        el("note-view").set_hidden(true);
        let _ = dialog().show_modal();
        let _ = editor().focus();
        caret_to_end();
    });
}

/// 只剩一個 <br> 之類的空殼時清空，讓提示文字出現
fn tidy_editor() {
    let e = editor();
    if e.inner_text().trim().is_empty() && e.query_selector("img").ok().flatten().is_none() {
        e.set_inner_html("");
    }
}

/// 把畫面上的內容存回去
async fn save_now() {
    let Some(id) = CURRENT.with(|c| c.borrow().clone()) else { return };
    tidy_editor();
    let e = editor();
    // 選取標記不要存進去
    let html = e.inner_html().replace(" class=\"sel\"", "").replace(" class=\"\"", "");
    let files = fids_in(&html);
    let note = Note { text: e.inner_text(), images: vec![], html, files };
    let key = note_key(&id);
    if note.is_empty() {
        chrome::remove(&[&key]).await;
    } else {
        chrome::set(&[(key.as_str(), to_js(&note))]).await;
    }
    let mut list: Vec<Todo> = chrome::get_or(KEY, vec![]).await;
    let before = list.clone();
    set_title(&mut list, &id, &title_input().value());
    set_has_note(&mut list, &id, !note.is_empty());
    if list != before {
        chrome::set(&[(KEY, to_js(&list))]).await;
    }
}

/// 打字停一下再存
fn save_soon() {
    let n = SAVE_SEQ.with(|s| {
        s.set(s.get() + 1);
        s.get()
    });
    timeout(400, move || {
        if SAVE_SEQ.with(|s| s.get()) == n {
            spawn(save_now());
        }
    });
}

fn close() {
    deselect();
    SAVE_SEQ.with(|s| s.set(s.get() + 1)); // 取消排定的儲存，直接存
    spawn(async {
        save_now().await;
        // 從筆記裡刪掉的檔案，關閉時才真的刪（編輯中還能 Ctrl+Z 復原）
        let keep = fids_in(&editor().inner_html());
        let gone: Vec<String> = SESSION_FILES.with(|s| s.take()).into_iter().filter(|f| !keep.contains(f)).map(|f| file_key(&f)).collect();
        if !gone.is_empty() {
            chrome::remove(&gone.iter().map(String::as_str).collect::<Vec<_>>()).await;
        }
        CURRENT.with(|c| *c.borrow_mut() = None);
    });
    if dialog().open() {
        dialog().close();
    }
}

// ---------- 游標與插入 ----------

fn html_doc() -> HtmlDocument {
    doc().unchecked_into()
}

/// 記住筆記區裡的游標位置
fn remember_caret() {
    let Some(sel) = web_sys::window().and_then(|w| w.get_selection().ok().flatten()) else { return };
    if sel.range_count() == 0 {
        return;
    }
    if let Ok(r) = sel.get_range_at(0) {
        if editor().contains(Some(&r.start_container().unwrap())) {
            CARET.with(|c| *c.borrow_mut() = Some(r.clone_range()));
        }
    }
}

fn caret_to_end() {
    let Some(sel) = web_sys::window().and_then(|w| w.get_selection().ok().flatten()) else { return };
    if let Ok(r) = doc().create_range() {
        let _ = r.select_node_contents(&editor());
        r.collapse_with_to_start(false);
        let _ = sel.remove_all_ranges();
        let _ = sel.add_range(&r);
    }
}

/// 回到筆記區上次的游標位置（沒有就放到最後）
fn restore_caret() {
    let _ = editor().focus();
    let saved = CARET.with(|c| c.borrow().clone());
    let Some(sel) = web_sys::window().and_then(|w| w.get_selection().ok().flatten()) else { return };
    match saved {
        Some(r) => {
            let _ = sel.remove_all_ranges();
            let _ = sel.add_range(&r);
        }
        None => caret_to_end(),
    }
}

/// 在游標位置插入（用 execCommand，才能 Ctrl+Z 復原）
fn insert_html(html: &str) {
    restore_caret();
    let _ = html_doc().exec_command_with_show_ui_and_value("insertHTML", false, html);
    remember_caret();
}

fn insert_text(text: &str) {
    let _ = html_doc().exec_command_with_show_ui_and_value("insertText", false, text);
}

// ---------- 圖片 ----------

/// 讀檔成 data: 網址
async fn read_data_url(file: &File) -> Option<String> {
    let reader = FileReader::new().ok()?;
    let r2 = reader.clone();
    let p = js_sys::Promise::new(&mut |resolve, _reject| {
        let r3 = r2.clone();
        let cb = Closure::once_into_js(move || {
            let _ = resolve.call1(&JsValue::NULL, &r3.result().unwrap_or(JsValue::NULL));
        });
        r2.set_onloadend(Some(cb.unchecked_ref()));
    });
    reader.read_as_data_url(file).ok()?;
    wasm_bindgen_futures::JsFuture::from(p).await.ok()?.as_string()
}

/// 太大的圖片縮小成 JPEG（避免塞爆儲存空間）
async fn shrink(data: String) -> String {
    let img = HtmlImageElement::new().unwrap();
    let i2 = img.clone();
    let p = js_sys::Promise::new(&mut |resolve, _reject| {
        let ok = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::NULL);
        });
        i2.set_onload(Some(ok.unchecked_ref()));
        i2.set_onerror(Some(ok.unchecked_ref()));
    });
    img.set_src(&data);
    let _ = wasm_bindgen_futures::JsFuture::from(p).await;
    let (w, h) = (img.natural_width() as f64, img.natural_height() as f64);
    if w == 0.0 || (w.max(h) <= MAX_SIDE && data.len() <= MAX_BYTES) {
        return data;
    }
    let scale = (MAX_SIDE / w.max(h)).min(1.0);
    let canvas: HtmlCanvasElement = doc().create_element("canvas").unwrap().unchecked_into();
    canvas.set_width((w * scale).round() as u32);
    canvas.set_height((h * scale).round() as u32);
    let Some(ctx) = canvas.get_context("2d").ok().flatten() else { return data };
    let ctx: web_sys::CanvasRenderingContext2d = ctx.unchecked_into();
    // 透明背景的 PNG 轉 JPEG 會變黑，先鋪白底
    ctx.set_fill_style_str("#fff");
    ctx.fill_rect(0.0, 0.0, w * scale, h * scale);
    let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(&img, 0.0, 0.0, w * scale, h * scale);
    canvas.to_data_url_with_type_and_encoder_options("image/jpeg", &JsValue::from_f64(0.85)).unwrap_or(data)
}

/// 在游標位置插入：一般圖片直接顯示在筆記裡，其他任何檔案變成附件標籤
fn add_files(files: Vec<File>) {
    if files.is_empty() {
        return;
    }
    msg("加入檔案中…");
    spawn(async move {
        let mut too_big = vec![];
        // 一次插入全部，順序才會跟選檔順序一樣
        let mut parts = vec![];
        for f in files {
            if inline_image(&f.type_()) {
                if let Some(d) = read_data_url(&f).await {
                    let d = shrink(d).await;
                    // data: 網址只有英數字和 +/=;:,，放進屬性很安全；保險起見還是去掉引號
                    parts.push(format!("<img src=\"{}\" alt=\"\"><br>", d.replace(['"', '<', '>'], "")));
                }
                continue;
            }
            if f.size() > MAX_FILE_BYTES {
                too_big.push(f.name());
                continue;
            }
            let Some(d) = read_data_url(&f).await else { continue };
            let fid = format!("{:x}{:06x}", chrome::now() as u64, (js_sys::Math::random() * 16_777_216.0) as u32);
            let mime = if f.type_().is_empty() { "application/octet-stream".to_string() } else { f.type_() };
            let file = NoteFile { name: f.name(), mime: mime.clone(), data: d };
            chrome::set(&[(file_key(&fid).as_str(), to_js(&file))]).await;
            SESSION_FILES.with(|s| s.borrow_mut().push(fid.clone()));
            parts.push(format!("{}&nbsp;", file_chip(&fid, &f.name(), f.size(), &mime)));
        }
        if !parts.is_empty() {
            insert_html(&parts.concat());
        }
        save_now().await;
        msg(&too_big.iter().map(|n| format!("檔案太大：{n}（上限 30 MB）")).collect::<Vec<_>>().join("\n"));
    });
}

/// 點筆記裡的附件：PDF、圖片、影音、文字檔在新分頁打開；其他類型直接下載
/// （Chrome 不能直接開 data: 網址，先轉成 blob:）
async fn open_file(fid: String) {
    let Some(f) = chrome::get::<NoteFile>(&file_key(&fid)).await else {
        return msg("找不到這個檔案（可能已被刪除）");
    };
    let win = web_sys::window().unwrap();
    let Ok(resp) = wasm_bindgen_futures::JsFuture::from(win.fetch_with_str(&f.data)).await else { return };
    let resp: web_sys::Response = resp.unchecked_into();
    let Ok(p) = resp.blob() else { return };
    let Ok(blob) = wasm_bindgen_futures::JsFuture::from(p).await else { return };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(blob.unchecked_ref()) else { return };
    if viewable(&f.name, &f.mime) {
        chrome::open_in_new_tab(&url, true).await;
    } else {
        let a: web_sys::HtmlAnchorElement = doc().create_element("a").unwrap().unchecked_into();
        a.set_href(&url);
        a.set_download(&f.name);
        a.click();
    }
}

fn zoom(img: &HtmlImageElement) {
    let view = el("note-view");
    view.query_selector("img").unwrap().unwrap().unchecked_into::<HtmlImageElement>().set_src(&img.src());
    view.set_hidden(false);
}

fn files_of(list: Option<web_sys::FileList>) -> Vec<File> {
    let Some(list) = list else { return vec![] };
    (0..list.length()).filter_map(|i| list.get(i)).collect()
}

pub fn start() {
    on_click("note-close", close);
    on_click("note-done", close);
    on_click("note-upload", || el("note-file").click());
    let ed = editor();
    listen(&ed, "input", |_| {
        remember_caret();
        save_soon();
    });
    for ev in ["keyup", "mouseup", "blur"] {
        listen(&ed, ev, |_| remember_caret());
    }
    listen(&el("note-title"), "input", |_| save_soon());
    listen(&el("note-title"), "keydown", |e| {
        let k: &KeyboardEvent = e.unchecked_ref();
        if k.key() == "Enter" && !k.is_composing() {
            e.prevent_default();
            let _ = editor().focus();
        }
    });
    listen(&el("note-file"), "change", |_| {
        let inp: HtmlInputElement = el("note-file").unchecked_into();
        add_files(files_of(inp.files()));
        inp.set_value("");
    });
    // 貼上：圖片直接放進筆記；文字一律用純文字貼（不帶其他網頁的格式）
    listen(&ed, "paste", |e| {
        let ev: &ClipboardEvent = e.unchecked_ref();
        let Some(dt) = ev.clipboard_data() else { return };
        e.prevent_default();
        let items = dt.items();
        let files: Vec<File> = (0..items.length())
            .filter_map(|i| items.get(i))
            .filter(|it| it.kind() == "file")
            .filter_map(|it| it.get_as_file().ok().flatten())
            .collect();
        if !files.is_empty() {
            remember_caret();
            add_files(files);
        } else if let Ok(t) = dt.get_data("text/plain") {
            insert_text(&t);
        }
    });
    // 主題欄位貼上圖片：也放進筆記
    listen(&el("note-title"), "paste", |e| {
        let ev: &ClipboardEvent = e.unchecked_ref();
        let Some(dt) = ev.clipboard_data() else { return };
        let files = files_of(dt.files());
        if !files.is_empty() {
            e.prevent_default();
            add_files(files);
        }
    });
    // 把圖片檔拖進視窗：放進筆記
    listen(&el("note-dialog"), "dragover", |e| {
        let ev: &DragEvent = e.unchecked_ref();
        if ev.data_transfer().is_some_and(|d| d.types().includes(&"Files".into(), 0)) {
            e.prevent_default();
        }
    });
    listen(&el("note-dialog"), "drop", |e| {
        let ev: &DragEvent = e.unchecked_ref();
        let Some(dt) = ev.data_transfer() else { return };
        let files = files_of(dt.files());
        if !files.is_empty() {
            e.prevent_default();
            add_files(files);
        } else if editor().contains(e.target().and_then(|t| t.dyn_into::<web_sys::Node>().ok()).as_ref()) {
            // 拖進來的文字也只收純文字
            e.prevent_default();
            if let Ok(t) = dt.get_data("text/plain") {
                let _ = editor().focus();
                insert_text(&t);
            }
        }
    });
    // 點筆記裡的圖片：選取（出現大小工具列和右下角拖曳點）；點兩下：放大檢視
    listen(&ed, "click", |e| {
        let target = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok());
        if let Some(chip) = target.as_ref().and_then(|t| t.closest(".note-file").ok().flatten()) {
            deselect();
            if let Some(fid) = chip.get_attribute("data-fid") {
                spawn(open_file(fid));
            }
            return;
        }
        match target.and_then(|t| t.dyn_into::<HtmlImageElement>().ok()) {
            Some(img) => select(img),
            None => deselect(),
        }
    });
    listen(&ed, "dblclick", |e| {
        if let Some(img) = e.target().and_then(|t| t.dyn_into::<HtmlImageElement>().ok()) {
            zoom(&img);
        }
    });
    listen(&ed, "scroll", |_| place_tools());
    listen(&el("note-dialog"), "scroll", |_| place_tools());
    listen(&web_sys::window().unwrap(), "resize", |_| {
        if is_open() {
            place_tools();
        }
    });
    // 選取圖片時按 Delete／Backspace：刪掉圖片；打其他字：取消選取
    listen(&ed, "keydown", |e| {
        let Some(img) = selected() else { return };
        let k: &KeyboardEvent = e.unchecked_ref();
        if matches!(k.key().as_str(), "Delete" | "Backspace") {
            e.prevent_default();
            img.remove();
            deselect();
            save_soon();
        } else {
            deselect();
        }
    });
    // 大小按鈕
    if let Ok(list) = el("note-imgbar").query_selector_all("button[data-w]") {
        for b in (0..list.length()).filter_map(|i| list.item(i)) {
            let b: web_sys::Element = b.unchecked_into();
            let pct: f64 = b.get_attribute("data-w").and_then(|w| w.parse().ok()).unwrap_or(0.0);
            listen(&b, "mousedown", |e| e.prevent_default()); // 不要讓筆記區失去游標
            listen(&b, "click", move |_| {
                if let Some(img) = selected() {
                    set_width(&img, pct);
                    // 等版面重排後再放工具列
                    timeout(0, place_tools);
                    save_soon();
                }
            });
        }
    }
    on_click("note-zoom", || {
        if let Some(img) = selected() {
            zoom(&img);
        }
    });
    on_click("note-imgdel", || {
        if let Some(img) = selected() {
            img.remove();
            deselect();
            save_soon();
        }
    });
    // 右下角拖曳點：自由調整寬度
    let handle = el("note-handle");
    listen(&handle, "pointerdown", |e| {
        let ev: &web_sys::PointerEvent = e.unchecked_ref();
        let Some(img) = selected() else { return };
        e.prevent_default();
        let _ = el("note-handle").set_pointer_capture(ev.pointer_id());
        RESIZING.with(|r| r.set(Some((ev.client_x() as f64, img.get_bounding_client_rect().width()))));
    });
    listen(&handle, "pointermove", |e| {
        let ev: &web_sys::PointerEvent = e.unchecked_ref();
        let (Some((x0, w0)), Some(img)) = (RESIZING.with(|r| r.get()), selected()) else { return };
        let w = (w0 + ev.client_x() as f64 - x0).max(40.0);
        set_width(&img, w / content_width() * 100.0);
        place_tools();
    });
    for ev in ["pointerup", "pointercancel"] {
        listen(&handle, ev, |_| {
            if RESIZING.with(|r| r.take()).is_some() {
                save_soon();
            }
        });
    }
    listen(&el("note-view"), "click", |_| el("note-view").set_hidden(true));
    // Esc：先關放大的圖片，再關視窗（都要存檔）
    listen(&el("note-dialog"), "cancel", |e| {
        e.prevent_default();
        if !el("note-view").hidden() {
            el("note-view").set_hidden(true);
        } else {
            close();
        }
    });
    listen(&el("note-dialog"), "click", |e| {
        if e.target().map(|t| t == el("note-dialog").into()).unwrap_or(false) {
            close();
        }
    });
}
