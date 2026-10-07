//! 右側書籤面板：顯示 Chrome 書籤，並可輸入名稱／網址／資料夾新增或編輯書籤

use crate::chrome::{self, to_js, BookmarkNode};
use crate::drag::{OnDrop, Pos, Sortable};
use std::rc::Rc;
use crate::ui::{doc, el, hide, listen, on_click, spawn};
use crate::videos::{normalize_url, Video};
use std::cell::{Cell, RefCell};
use wasm_bindgen::JsCast;
use web_sys::{
    Element, HtmlAnchorElement, HtmlButtonElement, HtmlDialogElement, HtmlImageElement, HtmlInputElement,
    HtmlOptionElement, HtmlSelectElement, KeyboardEvent, MouseEvent,
};

const PANEL_KEY: &str = "bmPanel"; // 面板開或關
const OPEN_KEY: &str = "bmOpen"; // 展開中的資料夾 id
const BAR_ID: &str = "1"; // Chrome 的「書籤列」
const LAST_FOLDER_KEY: &str = "bmLastFolder"; // 上次存到哪個資料夾
const PINS_KEY: &str = "bmPins"; // 標題列的 5 個釘選書籤
const PIN_SLOTS: usize = 5;
/// 加入書籤時「加入到資料夾最上方」上次的選擇
const TOP_FIRST_KEY: &str = "bmTopFirst";

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
struct Pin {
    url: String,
    title: String,
}

thread_local! {
    /// 目前畫面上的影片（「加入書籤」用）
    static CURRENT: RefCell<Option<Video>> = const { RefCell::new(None) };
    /// 「加入書籤」視窗正在編輯的書籤 id（新增時為 None）
    static EDITING: RefCell<Option<String>> = const { RefCell::new(None) };
    /// 剛移除的書籤（可復原）
    static LAST_REMOVED: RefCell<Option<BookmarkNode>> = const { RefCell::new(None) };
    static UNDO_SEQ: Cell<u32> = const { Cell::new(0) };
}

/// 影片換了：更新「加入書籤」按鈕
pub fn set_current(v: Option<Video>) {
    CURRENT.with(|c| *c.borrow_mut() = v);
    spawn(render_add_button());
}

fn current() -> Option<Video> {
    CURRENT.with(|c| c.borrow().clone())
}

// ---------- 加入書籤 ----------

async fn render_add_button() {
    let btn: HtmlButtonElement = el("bm-add").unchecked_into();
    let saved = match current() {
        Some(v) => !chrome::bookmarks_find_url(&v.watch_url()).await.is_empty(),
        None => false,
    };
    btn.set_text_content(Some(if saved { "★ 已加入書籤" } else { "☆ 加入書籤" }));
    btn.set_title(if saved { "編輯這部影片的書籤" } else { "新增書籤（預先填入目前這部影片）" });
    let _ = btn.class_list().toggle_with_force("on", saved);
}

/// 所有資料夾（含層級），給下拉選單用
fn collect_folders(n: &BookmarkNode, depth: usize, out: &mut Vec<(String, String, usize)>) {
    for c in n.children.iter().flatten().filter(|c| is_folder(c)) {
        let t = if c.title.trim().is_empty() { "（未命名資料夾）".to_string() } else { c.title.clone() };
        out.push((c.id.clone(), t, depth));
        collect_folders(c, depth + 1, out);
    }
}

/// 資料夾下拉選單（子資料夾縮排），並選好 `selected`
async fn fill_folders(selected: &str) {
    let mut folders = vec![];
    for r in chrome::bookmarks_tree().await.unwrap_or_default() {
        collect_folders(&r, 0, &mut folders);
    }
    let sel: HtmlSelectElement = el("bm-folder").unchecked_into();
    sel.set_inner_html("");
    for (id, t, depth) in &folders {
        let o: HtmlOptionElement = doc().create_element("option").unwrap().unchecked_into();
        o.set_value(id);
        o.set_text(&format!("{}{t}", "\u{3000}".repeat(*depth)));
        sel.append_child(&o).unwrap();
    }
    sel.set_value(if folders.iter().any(|f| f.0 == selected) { selected } else { BAR_ID });
}

// ---------- 新增資料夾 ----------

fn show_new_folder_row(show: bool) {
    hide("bm-nf-row", !show);
    hide("bm-newfolder", show);
    if show {
        let sel: HtmlSelectElement = el("bm-folder").unchecked_into();
        let parent = sel.selected_options().item(0).and_then(|o| o.text_content()).unwrap_or_default();
        el("bm-nf-hint").set_text_content(Some(&format!("會建立在「{}」裡", parent.trim())));
        input("bm-nf-name").set_value("");
        let _ = el("bm-nf-name").focus();
    }
}

/// 在目前選的資料夾裡建立新資料夾，並把書籤改放到新資料夾
async fn create_folder() {
    let name = input("bm-nf-name").value().trim().to_string();
    if name.is_empty() {
        let _ = el("bm-nf-name").focus();
        return;
    }
    let parent = el("bm-folder").unchecked_into::<HtmlSelectElement>().value();
    match chrome::bookmarks_create_folder(&parent, &name).await {
        Ok(id) => {
            fill_folders(&id).await;
            show_new_folder_row(false);
            // 展開父資料夾與新資料夾，存好後就看得到
            let mut open: Vec<String> = chrome::get_or(OPEN_KEY, default_open()).await;
            for f in [parent, id] {
                if !open.contains(&f) {
                    open.push(f);
                }
            }
            chrome::set(&[(OPEN_KEY, to_js(&open))]).await;
        }
        Err(e) => el("bm-error").set_text_content(Some(&format!("⚠ 無法建立資料夾：{e}"))),
    }
}

fn input(id: &str) -> HtmlInputElement {
    el(id).unchecked_into()
}

/// 打開「加入書籤」視窗：有目前影片就預先填入；這部影片已在書籤裡就變成編輯模式
async fn open_editor() {
    open_editor_for(current().map(|v| (v.title.clone(), v.watch_url()))).await;
}

/// 打開「加入書籤」視窗，預先填入 (名稱, 網址)；這個網址已經是書籤就變成編輯模式
/// （「最近」清單的 ☆ 也用這個）
pub(crate) async fn open_editor_for(prefill: Option<(String, String)>) {
    let existing = match &prefill {
        Some((_, url)) => chrome::bookmarks_find_url(url).await.into_iter().next(),
        None => None,
    };
    open_editor_with(existing, prefill).await;
}

/// 打開「編輯書籤」視窗，編輯這一個書籤（書籤列上每個書籤的 ✎）
async fn edit_bookmark(n: BookmarkNode) {
    open_editor_with(Some(n), None).await;
}

/// `existing`：要編輯的書籤（None 為新增）；`prefill`：新增時預先填入的 (名稱, 網址)
async fn open_editor_with(existing: Option<BookmarkNode>, prefill: Option<(String, String)>) {
    let last: String = chrome::get_or(LAST_FOLDER_KEY, BAR_ID.to_string()).await;
    let (title, url, folder) = match (&existing, prefill) {
        (Some(b), _) => (b.title.clone(), b.url.clone().unwrap_or_default(), b.parent_id.clone().unwrap_or(last)),
        (None, Some((t, u))) => (t, u, last),
        (None, None) => (String::new(), String::new(), last),
    };
    EDITING.with(|e| *e.borrow_mut() = existing.as_ref().map(|b| b.id.clone()));
    // 新增時沿用上次的選擇；編輯時預設不搬動位置
    let top_first: bool = chrome::get_or(TOP_FIRST_KEY, false).await;
    input("bm-top").set_checked(existing.is_none() && top_first);
    text_in(&el("bm-top-row"), ".bm-top-label", if existing.is_some() { "移到資料夾最上方" } else { "加入到資料夾最上方" });

    fill_folders(&folder).await;
    show_new_folder_row(false);

    let editing = existing.is_some();
    el("bm-d-title").set_text_content(Some(if editing { "編輯書籤" } else { "加入書籤" }));
    hide("bm-remove", !editing);
    input("bm-name").set_value(&title);
    input("bm-url").set_value(&url);
    el("bm-error").set_text_content(Some(""));
    let _ = dialog().show_modal();
    let _ = el(if url.is_empty() { "bm-url" } else { "bm-name" }).focus();
    input(if url.is_empty() { "bm-url" } else { "bm-name" }).select();
}

fn dialog() -> HtmlDialogElement {
    el("bm-dialog").unchecked_into()
}

async fn save_editor() {
    let Some(url) = normalize_url(&input("bm-url").value()) else {
        el("bm-error").set_text_content(Some("請輸入有效的網址，例如 https://www.google.com"));
        let _ = el("bm-url").focus();
        return;
    };
    let mut title = input("bm-name").value().trim().to_string();
    if title.is_empty() {
        title = url.clone();
    }
    let folder = el("bm-folder").unchecked_into::<HtmlSelectElement>().value();
    let top = input("bm-top").checked();
    let res = match EDITING.with(|e| e.borrow().clone()) {
        Some(id) => match chrome::bookmarks_edit(&id, &title, &url, &folder).await {
            Ok(()) if top => chrome::bookmarks_move(&id, &folder, Some(0)).await,
            r => r,
        },
        None => {
            chrome::set(&[(TOP_FIRST_KEY, to_js(&top))]).await;
            chrome::bookmarks_create(&folder, &title, &url, top.then_some(0)).await
        }
    };
    match res {
        Ok(()) => {
            chrome::set(&[(LAST_FOLDER_KEY, to_js(&folder))]).await;
            dialog().close();
            // 讓使用者看到剛加入的位置
            let mut open: Vec<String> = chrome::get_or(OPEN_KEY, default_open()).await;
            if !open.contains(&folder) {
                open.push(folder);
                chrome::set(&[(OPEN_KEY, to_js(&open))]).await;
            }
            render_tree().await;
        }
        Err(e) => el("bm-error").set_text_content(Some(&format!("⚠ {e}"))),
    }
    // 其餘畫面由 onCreated / onChanged 事件自動更新
}

async fn remove_editing() {
    if let Some(id) = EDITING.with(|e| e.borrow().clone()) {
        match chrome::bookmarks_remove(&id).await {
            Ok(()) => dialog().close(),
            Err(e) => el("bm-error").set_text_content(Some(&format!("⚠ {e}"))),
        }
    }
}

// ---------- 書籤樹 ----------

fn is_folder(n: &BookmarkNode) -> bool {
    n.url.is_none()
}

/// 所有書籤的網址（「最近」清單用來標示哪些已經加入書籤）
pub(crate) async fn all_urls() -> std::collections::HashSet<String> {
    fn walk(n: &BookmarkNode, out: &mut std::collections::HashSet<String>) {
        if let Some(u) = &n.url {
            out.insert(u.clone());
        }
        for c in n.children.iter().flatten() {
            walk(c, out);
        }
    }
    let mut out = std::collections::HashSet::new();
    for r in chrome::bookmarks_tree().await.unwrap_or_default() {
        walk(&r, &mut out);
    }
    out
}

/// 一列書籤：連結 + 右側 ✎ 編輯、✕ 移除
fn make_link(n: &BookmarkNode) -> Element {
    let row = doc().create_element("div").unwrap();
    row.set_class_name("bm-row");
    let a = make_anchor(n);
    row.append_child(&a).unwrap();
    crate::drag::sortable(
        Sortable { item: row.clone(), handle: Some(row.clone()), zone: row.clone(), group: "bm", id: n.id.clone(), can_contain: false },
        drop_handler(n, false),
    );
    // 點到列上的空白處也算點書籤
    let row2 = row.clone();
    listen(&row, "click", move |e| {
        if e.target().map(|t| t == row2.clone().into()).unwrap_or(false) {
            if let Some(a) = a.dyn_ref::<web_sys::HtmlElement>() {
                a.click();
            }
        }
    });

    let ed = doc().create_element("button").unwrap();
    ed.set_class_name("bm-del bm-edit");
    ed.set_text_content(Some("✎"));
    let _ = ed.set_attribute("type", "button");
    let _ = ed.set_attribute("title", "編輯書籤（名稱、網址、資料夾）");
    let _ = ed.set_attribute("aria-label", &format!("編輯書籤：{}", label_of(n)));
    let node = n.clone();
    listen(&ed, "click", move |e| {
        e.prevent_default();
        e.stop_propagation();
        spawn(edit_bookmark(node.clone()));
    });
    row.append_child(&ed).unwrap();

    let x = doc().create_element("button").unwrap();
    x.set_class_name("bm-del");
    x.set_text_content(Some("✕"));
    let _ = x.set_attribute("type", "button");
    let _ = x.set_attribute("title", "移除書籤");
    let _ = x.set_attribute("aria-label", &format!("移除書籤：{}", label_of(n)));
    let node = n.clone();
    listen(&x, "click", move |e| {
        e.prevent_default();
        e.stop_propagation();
        let node = node.clone();
        spawn(async move { remove_with_undo(node).await });
    });
    row.append_child(&x).unwrap();
    row
}

fn label_of(n: &BookmarkNode) -> String {
    if n.title.trim().is_empty() { n.url.clone().unwrap_or_default() } else { n.title.clone() }
}

fn make_anchor(n: &BookmarkNode) -> Element {
    link_anchor(&n.url.clone().unwrap_or_default(), &label_of(n))
}

/// 一個可點的連結列（網站圖示 + 名稱），書籤與「最近」共用
pub(crate) fn link_anchor(url: &str, label: &str) -> Element {
    let (url, label) = (url.to_string(), label.to_string());
    let a: HtmlAnchorElement = doc().create_element("a").unwrap().unchecked_into();
    a.set_class_name("bm-item");
    a.set_inner_html(r#"<img alt=""><span></span>"#);
    a.query_selector("span").unwrap().unwrap().set_text_content(Some(&label));
    let img: HtmlImageElement = a.query_selector("img").unwrap().unwrap().unchecked_into();
    img.set_src(&chrome::favicon_url(&url));

    if url.starts_with("javascript:") {
        // 書籤小程式只能在一般網頁上執行
        a.set_title("書籤小程式無法在新分頁執行");
        let _ = a.class_list().add_1("disabled");
        return a.into();
    }
    a.set_href(&url);
    a.set_title(&format!("{label}\n{url}"));
    // 一律用 chrome.tabs 開啟，不靠 <a> 的預設跳轉（新分頁覆寫頁面上不一定可靠，chrome:// 也打不開）
    // 一般點擊：這個分頁；Ctrl/⌘ 或中鍵：背景新分頁；Shift：新分頁並切過去
    let u = url.clone();
    listen(&a, "click", move |e| {
        e.prevent_default();
        let m: &MouseEvent = e.unchecked_ref();
        let url = u.clone();
        let (new_tab, active) = (m.ctrl_key() || m.meta_key() || m.shift_key(), m.shift_key());
        spawn(async move {
            if new_tab {
                chrome::open_in_new_tab(&url, active).await
            } else {
                chrome::open_in_this_tab(&url).await
            }
        });
    });
    let u = url.clone();
    listen(&a, "auxclick", move |e| {
        if e.unchecked_ref::<MouseEvent>().button() == 1 {
            e.prevent_default();
            let url = u.clone();
            spawn(async move { chrome::open_in_new_tab(&url, false).await });
        }
    });
    a.into()
}

// ---------- 拖曳排序 ----------

/// 有書籤被拖到 `target` 上放開
fn drop_handler(target: &BookmarkNode, is_root: bool) -> OnDrop {
    let (tid, parent, index) = (target.id.clone(), target.parent_id.clone(), target.index.unwrap_or(0));
    Rc::new(move |from, _to, pos| {
        let (tid, parent) = (tid.clone(), parent.clone());
        spawn(async move {
            // 從標題列的釘選格拖下來：放到這個位置，並從釘選格移除
            if let Some(slot) = from.strip_prefix("pin:").and_then(|s| s.parse::<usize>().ok()) {
                let (dest, at) = if pos == Pos::Into || is_root {
                    (tid.clone(), None)
                } else {
                    (parent.clone().unwrap_or_else(|| BAR_ID.to_string()), Some(index + u32::from(pos == Pos::After)))
                };
                return drop_pin_into_tree(slot, &dest, at).await;
            }
            let res = if pos == Pos::Into || is_root {
                // 放進資料夾（排在最後），並展開它
                let mut open: Vec<String> = chrome::get_or(OPEN_KEY, default_open()).await;
                if !open.contains(&tid) {
                    open.push(tid.clone());
                    chrome::set(&[(OPEN_KEY, to_js(&open))]).await;
                }
                chrome::bookmarks_move(&from, &tid, None).await
            } else {
                // chrome.bookmarks.move 的 index 是「移動前」的位置，所以放到後面就是 index + 1
                let i = index + u32::from(pos == Pos::After);
                chrome::bookmarks_move(&from, parent.as_deref().unwrap_or(BAR_ID), Some(i)).await
            };
            if let Err(e) = res {
                show_undo_bar(&format!("⚠ 無法移動：{e}"), false);
            }
            // 畫面由 onMoved 事件自動更新
        });
    })
}

/// 釘選格拖到書籤列：已經是書籤就搬到那裡，否則在那裡新增；然後空出釘選格
async fn drop_pin_into_tree(slot: usize, dest: &str, at: Option<u32>) {
    let mut pins = load_pins().await;
    let Some(pin) = pins.get(slot).cloned().flatten() else { return };
    let res = match chrome::bookmarks_find_url(&pin.url).await.into_iter().next() {
        Some(b) => chrome::bookmarks_move(&b.id, dest, at).await,
        None => {
            let title = if pin.title.trim().is_empty() { pin.url.clone() } else { pin.title.clone() };
            chrome::bookmarks_create(dest, &title, &pin.url, at).await
        }
    };
    match res {
        Ok(()) => {
            pins[slot] = None;
            chrome::set(&[(PINS_KEY, to_js(&pins))]).await;
            render_pins().await;
            let mut open: Vec<String> = chrome::get_or(OPEN_KEY, default_open()).await;
            if !open.iter().any(|o| o == dest) {
                open.push(dest.to_string());
                chrome::set(&[(OPEN_KEY, to_js(&open))]).await;
            }
        }
        Err(e) => show_undo_bar(&format!("⚠ 無法放進書籤：{e}"), false),
    }
}

// ---------- 移除與復原 ----------

async fn remove_with_undo(n: BookmarkNode) {
    if let Err(e) = chrome::bookmarks_remove(&n.id).await {
        return show_undo_bar(&format!("⚠ 無法移除：{e}"), false);
    }
    let label = label_of(&n);
    LAST_REMOVED.with(|r| *r.borrow_mut() = Some(n));
    show_undo_bar(&format!("已移除「{label}」"), true);
}

fn show_undo_bar(msg: &str, can_undo: bool) {
    el("bm-undo-msg").set_text_content(Some(msg));
    hide("bm-undo-btn", !can_undo);
    hide("bm-undo", false);
    let seq = UNDO_SEQ.with(|s| {
        s.set(s.get() + 1);
        s.get()
    });
    crate::ui::timeout(6000, move || {
        // 期間又移除了別的，就交給新的計時
        if UNDO_SEQ.with(|s| s.get()) == seq {
            hide("bm-undo", true);
            LAST_REMOVED.with(|r| *r.borrow_mut() = None);
        }
    });
}

async fn undo_remove() {
    let Some(n) = LAST_REMOVED.with(|r| r.borrow_mut().take()) else { return };
    hide("bm-undo", true);
    let parent = n.parent_id.clone().unwrap_or_else(|| BAR_ID.into());
    let url = n.url.clone().unwrap_or_default();
    if let Err(e) = chrome::bookmarks_create(&parent, &n.title, &url, n.index).await {
        show_undo_bar(&format!("⚠ 無法復原：{e}"), false);
    }
}

fn count_links(n: &BookmarkNode) -> usize {
    n.children.as_ref().map_or(0, |c| c.iter().map(|x| if is_folder(x) { count_links(x) } else { 1 }).sum())
}

fn make_folder(n: &BookmarkNode, open: &[String], is_root: bool) -> Element {
    let d = doc().create_element("details").unwrap();
    d.set_class_name("bm-folder");
    if open.contains(&n.id) {
        let _ = d.set_attribute("open", "");
    }
    d.set_inner_html(r#"<summary><span class="bm-chev" aria-hidden="true"></span><span class="bm-ftitle"></span><span class="n"></span></summary><div class="bm-children"></div>"#);
    let t = if n.title.trim().is_empty() { "（未命名資料夾）" } else { n.title.as_str() };
    d.query_selector(".bm-ftitle").unwrap().unwrap().set_text_content(Some(t));
    d.query_selector(".n").unwrap().unwrap().set_text_content(Some(&count_links(n).to_string()));

    // 拖曳：資料夾列上半／下半 = 放到前面／後面，中間 = 放進資料夾；書籤列等最上層只能放進去
    let summary = d.query_selector("summary").unwrap().unwrap();
    crate::drag::sortable(
        Sortable {
            item: d.clone(),
            handle: if is_root { None } else { Some(summary.clone()) },
            zone: summary,
            group: "bm",
            id: n.id.clone(),
            can_contain: true,
        },
        drop_handler(n, is_root),
    );

    let box_ = d.query_selector(".bm-children").unwrap().unwrap();
    append_children(&box_, n, open);

    let id = n.id.clone();
    let d2 = d.clone();
    listen(&d, "toggle", move |e| {
        // <details> 的 toggle 事件不會冒泡，但保險起見只處理自己的
        if e.target().map(|t| t != d2.clone().into()).unwrap_or(true) {
            return;
        }
        let id = id.clone();
        let is_open = d2.has_attribute("open");
        spawn(async move {
            let mut list: Vec<String> = chrome::get_or(OPEN_KEY, default_open()).await;
            list.retain(|x| x != &id);
            if is_open {
                list.push(id);
            }
            chrome::set(&[(OPEN_KEY, to_js(&list))]).await;
        });
    });
    d
}

fn append_children(parent: &Element, n: &BookmarkNode, open: &[String]) {
    for c in n.children.iter().flatten() {
        let node = if is_folder(c) { make_folder(c, open, false) } else { make_link(c) };
        parent.append_child(&node).unwrap();
    }
}

fn default_open() -> Vec<String> {
    vec![BAR_ID.into()]
}

async fn render_tree() {
    let box_ = el("bm-tree");
    // 有搜尋文字：改顯示符合的書籤（平鋪）
    let q = crate::ui::search_text("bm-search");
    if !q.is_empty() {
        let found = chrome::bookmarks_search(&q).await;
        box_.set_inner_html("");
        if found.is_empty() {
            box_.set_inner_html(r#"<div class="bm-none">找不到符合的書籤</div>"#);
        }
        for n in &found {
            box_.append_child(&make_link(n)).unwrap();
        }
        return;
    }
    let roots = match chrome::bookmarks_tree().await {
        Ok(r) => r,
        Err(e) => {
            box_.set_inner_html(r#"<div class="bm-none"></div>"#);
            text_in(&box_, ".bm-none", &format!("⚠ 讀不到書籤：{e}"));
            return;
        }
    };
    let open: Vec<String> = chrome::get_or(OPEN_KEY, default_open()).await;
    box_.set_inner_html("");
    // 最上層：書籤列、其他書籤、行動裝置書籤（空的就不顯示）
    let tops: Vec<&BookmarkNode> = roots
        .iter()
        .flat_map(|r| r.children.iter().flatten())
        .filter(|n| n.children.as_ref().is_some_and(|c| !c.is_empty()))
        .collect();
    if tops.is_empty() {
        box_.set_inner_html(r#"<div class="bm-none">還沒有書籤。<br>按上方「☆ 加入書籤」新增。</div>"#);
        return;
    }
    for n in tops {
        let f = make_folder(n, &open, true);
        let _ = f.class_list().add_1("bm-root");
        box_.append_child(&f).unwrap();
    }
}

fn text_in(parent: &Element, sel: &str, s: &str) {
    if let Some(e) = parent.query_selector(sel).ok().flatten() {
        e.set_text_content(Some(s));
    }
}

// ---------- 標題列的 5 個釘選書籤 ----------

async fn load_pins() -> Vec<Option<Pin>> {
    let mut v: Vec<Option<Pin>> = chrome::get_or(PINS_KEY, vec![]).await;
    v.resize(PIN_SLOTS, None);
    v
}

async fn set_pin(i: usize, pin: Option<Pin>) {
    let mut v = load_pins().await;
    v[i] = pin;
    chrome::set(&[(PINS_KEY, to_js(&v))]).await;
    render_pins().await;
}

async fn render_pins() {
    let pins = load_pins().await;
    for (i, p) in pins.iter().enumerate() {
        let slot = el(&format!("bm-pin-{i}"));
        slot.set_inner_html("");
        match p {
            Some(p) => {
                let _ = slot.set_attribute("draggable", "true");
                let _ = slot.class_list().remove_1("pin-empty");
                let label = if p.title.trim().is_empty() { p.url.as_str() } else { p.title.as_str() };
                let a = link_anchor(&p.url, label);
                slot.append_child(&a).unwrap();
                let x = doc().create_element("button").unwrap();
                x.set_class_name("pin-x");
                x.set_text_content(Some("✕"));
                let _ = x.set_attribute("type", "button");
                let _ = x.set_attribute("title", "取消釘選");
                let _ = x.set_attribute("aria-label", &format!("取消釘選：{label}"));
                listen(&x, "click", move |e| {
                    e.prevent_default();
                    e.stop_propagation();
                    spawn(set_pin(i, None));
                });
                slot.append_child(&x).unwrap();
                slot.set_title(&format!("{label}\n{}", p.url));
            }
            None => {
                let _ = slot.set_attribute("draggable", "false");
                let _ = slot.class_list().add_1("pin-empty");
                slot.set_text_content(Some("+"));
                slot.set_title("把書籤拖到這裡釘選");
            }
        }
    }
}

fn setup_pins() {
    for i in 0..PIN_SLOTS {
        let slot: Element = el(&format!("bm-pin-{i}")).unchecked_into();
        // 書籤列拖到格子上放開 = 釘選（資料夾不行）；釘選格之間拖曳 = 交換位置；
        // 釘選格也可以往下拖到書籤列（見 drop_handler）
        crate::drag::sortable(
            Sortable { item: slot.clone(), handle: Some(slot.clone()), zone: slot.clone(), group: "bm", id: format!("pin:{i}"), can_contain: false },
            Rc::new(move |from, _to, _pos| {
                spawn(async move {
                    if let Some(j) = from.strip_prefix("pin:").and_then(|s| s.parse::<usize>().ok()) {
                        let mut v = load_pins().await;
                        crate::apps::swap_pins(&mut v, j, i);
                        chrome::set(&[(PINS_KEY, to_js(&v))]).await;
                        return render_pins().await;
                    }
                    match chrome::bookmark_get(&from).await {
                        Some(n) if n.url.is_some() => {
                            set_pin(i, Some(Pin { url: n.url.unwrap_or_default(), title: n.title })).await;
                        }
                        Some(_) => show_undo_bar("資料夾不能釘選，請拖一個書籤", false),
                        None => {}
                    }
                })
            }),
        );
    }
    spawn(render_pins());
}

// ---------- 面板開關 ----------

fn apply_panel(open: bool) {
    hide("bm", !open);
    let _ = doc().body().unwrap().class_list().toggle_with_force("bm-open", open);
    let _ = el("bm-toggle").class_list().toggle_with_force("on", open);
    el("bm-toggle").set_attribute("aria-expanded", if open { "true" } else { "false" }).ok();
}

/// 被別的面板打開時關掉書籤
pub fn close() {
    if !el("bm").hidden() {
        spawn(set_panel(false));
    }
}

async fn set_panel(open: bool) {
    if open {
        crate::ui::close_panels_except("bm");
    }
    apply_panel(open);
    chrome::set(&[(PANEL_KEY, to_js(&open))]).await;
    if open {
        render_tree().await;
    }
}

pub fn start() {
    on_click("bm-toggle", || {
        let open = el("bm").hidden();
        spawn(set_panel(open));
    });
    on_click("bm-close", || spawn(set_panel(false)));
    setup_pins();
    crate::ui::search_box("bm-search", || spawn(render_tree()));
    on_click("bm-add", || spawn(open_editor()));
    on_click("bm-undo-btn", || spawn(undo_remove()));
    on_click("bm-save", || spawn(save_editor()));
    on_click("bm-remove", || spawn(remove_editing()));
    on_click("bm-cancel", || dialog().close());
    on_click("bm-newfolder", || show_new_folder_row(true));
    on_click("bm-nf-cancel", || show_new_folder_row(false));
    on_click("bm-nf-ok", || spawn(create_folder()));
    listen(&el("bm-nf-name"), "keydown", |e| {
        let k: &KeyboardEvent = e.unchecked_ref();
        if k.is_composing() {
            return;
        }
        match k.key().as_str() {
            "Enter" => {
                e.prevent_default();
                spawn(create_folder());
            }
            "Escape" => {
                // 只收起新增資料夾，不要關掉整個視窗
                e.prevent_default();
                e.stop_propagation();
                show_new_folder_row(false);
            }
            _ => {}
        }
    });
    on_click("bm-d-close", || dialog().close());
    for id in ["bm-name", "bm-url"] {
        listen(&el(id), "keydown", |e| {
            let k: &KeyboardEvent = e.unchecked_ref();
            if k.key() == "Enter" && !k.is_composing() {
                e.prevent_default();
                spawn(save_editor());
            }
        });
    }
    listen(&el("bm-dialog"), "click", |e| {
        if e.target().map(|t| t == el("bm-dialog").into()).unwrap_or(false) {
            dialog().close();
        }
    });

    chrome::on_bookmarks_changed(|| {
        spawn(async {
            if !el("bm").hidden() {
                render_tree().await;
            }
            render_add_button().await;
        })
    });

    spawn(async {
        // 已經有別的面板開著（例如從提醒視窗打開 TODO）就不要再打開書籤
        let others_open = !el("todo").hidden() || !el("recent").hidden();
        let open: bool = chrome::get_or(PANEL_KEY, true).await && !others_open;
        apply_panel(open);
        if open {
            render_tree().await;
        }
        render_add_button().await;
    });
}

