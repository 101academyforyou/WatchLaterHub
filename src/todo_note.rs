//! TODO 的 📝 筆記小視窗：改主題、寫更多筆記，圖片直接貼在筆記裡（文字和圖片混排）
//!
//! 筆記存在 `todoNote:<id>`（chrome.storage），清單只記 `hasNote`。
//! 打字停 0.4 秒自動存；圖片超過 1600px 或太大時縮小成 JPEG。
//! 貼上時只收純文字和圖片，不收其他網頁的格式，所以存下來的 HTML 只有文字、換行和 <img>。

use crate::chrome::{self, to_js};
use crate::todo::{note_key, set_has_note, set_title, Note, Todo, KEY};
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
        CURRENT.with(|c| *c.borrow_mut() = Some(id));
        CARET.with(|c| *c.borrow_mut() = None);
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
    let note = Note { text: e.inner_text(), images: vec![], html: e.inner_html() };
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
    SAVE_SEQ.with(|s| s.set(s.get() + 1)); // 取消排定的儲存，直接存
    spawn(async {
        save_now().await;
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

/// 在游標位置插入圖片
fn add_files(files: Vec<File>) {
    let files: Vec<File> = files.into_iter().filter(|f| f.type_().starts_with("image/")).collect();
    if files.is_empty() {
        return;
    }
    msg("加入圖片中…");
    spawn(async move {
        for f in files {
            if let Some(d) = read_data_url(&f).await {
                let d = shrink(d).await;
                // data: 網址只有英數字和 +/=;:,，放進屬性很安全；保險起見還是去掉引號
                insert_html(&format!("<img src=\"{}\" alt=\"\"><br>", d.replace(['"', '<', '>'], "")));
            }
        }
        save_now().await;
        msg("");
    });
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
            .filter(|it| it.kind() == "file" && it.type_().starts_with("image/"))
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
    // 點筆記裡的圖片：放大
    listen(&ed, "click", |e| {
        if let Some(img) = e.target().and_then(|t| t.dyn_into::<HtmlImageElement>().ok()) {
            let view = el("note-view");
            view.query_selector("img").unwrap().unwrap().unchecked_into::<HtmlImageElement>().set_src(&img.src());
            view.set_hidden(false);
        }
    });
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
