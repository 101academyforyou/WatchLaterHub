//! TODO 的 📝 筆記小視窗：改主題、寫更多筆記、貼上／上傳／拖進圖片
//!
//! 筆記存在 `todoNote:<id>`（chrome.storage），清單只記 `hasNote`。
//! 打字停 0.4 秒自動存；圖片超過 1600px 或太大時縮小成 JPEG。

use crate::chrome::{self, to_js};
use crate::todo::{note_key, set_has_note, set_title, Note, Todo, KEY};
use crate::ui::{doc, el, listen, on_click, spawn, timeout};
use std::cell::{Cell, RefCell};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    ClipboardEvent, DragEvent, File, FileReader, HtmlCanvasElement, HtmlDialogElement, HtmlImageElement, HtmlInputElement,
    HtmlTextAreaElement, KeyboardEvent,
};

const MAX_SIDE: f64 = 1600.0;
const MAX_BYTES: usize = 1_500_000;

thread_local! {
    /// 正在編輯的 (待辦 id, 筆記)
    static CURRENT: RefCell<Option<(String, Note)>> = const { RefCell::new(None) };
    static SAVE_SEQ: Cell<u32> = const { Cell::new(0) };
}

fn dialog() -> HtmlDialogElement {
    el("note-dialog").unchecked_into()
}

fn title_input() -> HtmlInputElement {
    el("note-title").unchecked_into()
}

fn text_area() -> HtmlTextAreaElement {
    el("note-text").unchecked_into()
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
        text_area().set_value(&note.text);
        CURRENT.with(|c| *c.borrow_mut() = Some((id, note)));
        msg("");
        el("note-view").set_hidden(true);
        render_images();
        let _ = dialog().show_modal();
        let _ = text_area().focus();
    });
}

/// 把畫面上的內容存回去
async fn save_now() {
    let Some((id, mut note)) = CURRENT.with(|c| c.borrow().clone()) else { return };
    note.text = text_area().value();
    CURRENT.with(|c| *c.borrow_mut() = Some((id.clone(), note.clone())));
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

fn render_images() {
    let box_ = el("note-imgs");
    box_.set_inner_html("");
    let images = CURRENT.with(|c| c.borrow().as_ref().map(|(_, n)| n.images.clone()).unwrap_or_default());
    box_.set_hidden(images.is_empty());
    for (i, src) in images.into_iter().enumerate() {
        let fig = doc().create_element("div").unwrap();
        fig.set_class_name("note-img");
        fig.set_inner_html(r#"<img alt=""><button class="bm-del" type="button" title="移除圖片" aria-label="移除圖片">✕</button>"#);
        let img: HtmlImageElement = fig.query_selector("img").unwrap().unwrap().unchecked_into();
        img.set_src(&src);
        let _ = img.set_attribute("title", "點一下放大");
        listen(&img, "click", move |_| {
            let view = el("note-view");
            view.query_selector("img").unwrap().unwrap().unchecked_into::<HtmlImageElement>().set_src(&src);
            view.set_hidden(false);
        });
        listen(&fig.query_selector("button").unwrap().unwrap(), "click", move |_| {
            CURRENT.with(|c| {
                if let Some((_, n)) = c.borrow_mut().as_mut() {
                    if i < n.images.len() {
                        n.images.remove(i);
                    }
                }
            });
            render_images();
            spawn(save_now());
        });
        box_.append_child(&fig).unwrap();
    }
}

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
                CURRENT.with(|c| {
                    if let Some((_, n)) = c.borrow_mut().as_mut() {
                        n.images.push(d);
                    }
                });
            }
        }
        render_images();
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
    listen(&el("note-text"), "input", |_| save_soon());
    listen(&el("note-title"), "input", |_| save_soon());
    listen(&el("note-title"), "keydown", |e| {
        let k: &KeyboardEvent = e.unchecked_ref();
        if k.key() == "Enter" && !k.is_composing() {
            e.prevent_default();
            let _ = text_area().focus();
        }
    });
    listen(&el("note-file"), "change", |_| {
        let inp: HtmlInputElement = el("note-file").unchecked_into();
        add_files(files_of(inp.files()));
        inp.set_value("");
    });
    // 貼上圖片（純文字照常貼進筆記）
    listen(&el("note-dialog"), "paste", |e| {
        let ev: &ClipboardEvent = e.unchecked_ref();
        let Some(dt) = ev.clipboard_data() else { return };
        let items = dt.items();
        let files: Vec<File> = (0..items.length())
            .filter_map(|i| items.get(i))
            .filter(|it| it.kind() == "file" && it.type_().starts_with("image/"))
            .filter_map(|it| it.get_as_file().ok().flatten())
            .collect();
        if !files.is_empty() {
            e.prevent_default();
            add_files(files);
        }
    });
    // 把圖片檔拖進視窗
    listen(&el("note-dialog"), "dragover", |e| {
        let ev: &DragEvent = e.unchecked_ref();
        if ev.data_transfer().is_some_and(|d| d.types().includes(&"Files".into(), 0)) {
            e.prevent_default();
        }
    });
    listen(&el("note-dialog"), "drop", |e| {
        let ev: &DragEvent = e.unchecked_ref();
        let files = files_of(ev.data_transfer().and_then(|d| d.files()));
        if !files.is_empty() {
            e.prevent_default();
            add_files(files);
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
