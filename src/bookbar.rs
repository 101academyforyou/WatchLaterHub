//! 頂列左側的書籤列（跟 Chrome 的書籤列一樣）：
//! 書籤列裡的書籤橫向排開，放不下的收進「»」；資料夾點開是下拉選單

use crate::bookmarks::link_anchor;
use crate::chrome::{self, BookmarkNode};
use crate::ui::{doc, el, listen, spawn};
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, KeyboardEvent};

const BAR_ID: &str = "1"; // Chrome 的「書籤列」

thread_local! {
    /// 書籤列最上層的項目（「»」選單要用）
    static ITEMS: RefCell<Vec<BookmarkNode>> = const { RefCell::new(vec![]) };
}

/// 書籤列裡的項目。登入 Chrome 同步書籤時會有兩個書籤列（本機的 id "1" 和帳號的），
/// 都用 folderType 認出來後合在一起，帳號的排前面（跟 Chrome 顯示的順序一樣）
pub fn bar_items(roots: &[BookmarkNode]) -> Vec<BookmarkNode> {
    let mut bars: Vec<&BookmarkNode> = roots
        .iter()
        .flat_map(|r| r.children.iter().flatten())
        .filter(|n| n.folder_type.as_deref() == Some("bookmarks-bar") || (n.folder_type.is_none() && n.id == BAR_ID))
        .collect();
    bars.sort_by_key(|n| n.id == BAR_ID);
    bars.into_iter().flat_map(|b| b.children.clone().unwrap_or_default()).collect()
}

fn is_folder(n: &BookmarkNode) -> bool {
    n.url.is_none()
}

fn label(n: &BookmarkNode) -> String {
    if n.title.trim().is_empty() { n.url.clone().unwrap_or_default() } else { n.title.clone() }
}

/// 資料夾按鈕（書籤列與選單共用）
fn folder_button(n: &BookmarkNode, class: &str) -> Element {
    let b = doc().create_element("button").unwrap();
    b.set_class_name(class);
    let _ = b.set_attribute("type", "button");
    let _ = b.set_attribute("title", &n.title);
    b.set_inner_html(r#"<span class="bb-ico" aria-hidden="true">📁</span><span class="bb-t"></span>"#);
    b.query_selector(".bb-t").unwrap().unwrap().set_text_content(Some(&label(n)));
    b
}

async fn render() {
    let bar = el("bbar");
    let roots = chrome::bookmarks_tree().await.unwrap_or_default();
    let items = bar_items(&roots);
    bar.set_inner_html("");
    for n in &items {
        let e = if is_folder(n) {
            let b = folder_button(n, "bb-item bb-folder");
            let node = n.clone();
            let anchor = b.clone();
            listen(&b, "click", move |e| {
                e.stop_propagation();
                toggle_menu(&anchor, node.children.clone().unwrap_or_default());
            });
            b
        } else {
            let a = link_anchor(&n.url.clone().unwrap_or_default(), &label(n));
            a.set_class_name("bb-item");
            a
        };
        bar.append_child(&e).unwrap();
    }
    ITEMS.with(|i| *i.borrow_mut() = items);
    fit();
}

/// 放不下的項目換到看不見的第二行；有的話顯示「»」
fn fit() {
    let bar = el("bbar");
    let kids = bar.children();
    let first_top = kids.item(0).map(|k| k.unchecked_into::<HtmlElement>().offset_top());
    let overflow = (0..kids.length())
        .filter_map(|i| kids.item(i))
        .any(|k| Some(k.unchecked_into::<HtmlElement>().offset_top()) != first_top);
    el("bbar-more").set_hidden(!overflow);
}

/// 「»」選單：書籤列放不下的那些
fn overflow_items() -> Vec<BookmarkNode> {
    let bar = el("bbar");
    let kids = bar.children();
    let first_top = kids.item(0).map(|k| k.unchecked_into::<HtmlElement>().offset_top());
    let all = ITEMS.with(|i| i.borrow().clone());
    all.into_iter()
        .enumerate()
        .filter(|(i, _)| {
            kids.item(*i as u32).map(|k| Some(k.unchecked_into::<HtmlElement>().offset_top()) != first_top).unwrap_or(false)
        })
        .map(|(_, n)| n)
        .collect()
}

fn menu() -> HtmlElement {
    el("bb-menu")
}

fn close_menu() {
    menu().set_hidden(true);
    for e in crate::ui::all("[data-key][aria-expanded]") {
        let _ = e.remove_attribute("aria-expanded");
    }
    let _ = menu().remove_attribute("data-for");
}

/// 在 `anchor` 下方打開（或關閉）選單
fn toggle_menu(anchor: &Element, items: Vec<BookmarkNode>) {
    let m = menu();
    let key = anchor.get_attribute("data-key").unwrap_or_else(|| {
        let k = format!("{}", js_sys::Math::random());
        let _ = anchor.set_attribute("data-key", &k);
        k
    });
    if !m.hidden() && m.get_attribute("data-for").as_deref() == Some(&key) {
        close_menu();
        return;
    }
    close_menu();
    let _ = m.set_attribute("data-for", &key);
    let _ = anchor.set_attribute("aria-expanded", "true");
    fill_menu(anchor, items, vec![]);
    let r = anchor.get_bounding_client_rect();
    let win = web_sys::window().unwrap();
    let vw = win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1200.0);
    m.set_hidden(false);
    let w = m.offset_width() as f64;
    let left = r.left().min(vw - w - 8.0).max(8.0);
    let _ = m.style().set_property("left", &format!("{left}px"));
    let _ = m.style().set_property("top", &format!("{}px", r.bottom() + 4.0));
}

/// `parents`：一路點進來的上層清單（「‹ 上一層」用）
fn fill_menu(anchor: &Element, items: Vec<BookmarkNode>, parents: Vec<Vec<BookmarkNode>>) {
    let m = menu();
    m.set_inner_html("");
    m.set_scroll_top(0);
    if !parents.is_empty() {
        let b = doc().create_element("button").unwrap();
        b.set_class_name("bb-m-item bb-back");
        let _ = b.set_attribute("type", "button");
        b.set_text_content(Some("‹ 上一層"));
        let (anchor, parents) = (anchor.clone(), parents.clone());
        listen(&b, "click", move |e| {
            e.stop_propagation();
            let mut p = parents.clone();
            let up = p.pop().unwrap_or_default();
            fill_menu(&anchor, up, p);
        });
        m.append_child(&b).unwrap();
    }
    if items.is_empty() {
        let d = doc().create_element("div").unwrap();
        d.set_class_name("bb-empty");
        d.set_text_content(Some("（空的）"));
        m.append_child(&d).unwrap();
    }
    for n in &items {
        let e = if is_folder(n) {
            let b = folder_button(n, "bb-m-item bb-m-folder");
            let (anchor, node) = (anchor.clone(), n.clone());
            let mut up = parents.clone();
            up.push(items.clone());
            listen(&b, "click", move |e| {
                e.stop_propagation();
                fill_menu(&anchor, node.children.clone().unwrap_or_default(), up.clone());
            });
            b
        } else {
            let a = link_anchor(&n.url.clone().unwrap_or_default(), &label(n));
            let _ = a.class_list().add_1("bb-m-item");
            listen(&a, "click", |_| close_menu());
            a
        };
        m.append_child(&e).unwrap();
    }
}

pub fn start() {
    let more = el("bbar-more");
    let anchor: Element = more.clone().into();
    listen(&more, "click", move |e| {
        e.stop_propagation();
        toggle_menu(&anchor, overflow_items());
    });
    // 點選單外面、按 Esc、捲動或改變視窗大小：關閉選單
    listen(&doc(), "click", |e| {
        let inside = e.target().and_then(|t| t.dyn_into::<web_sys::Node>().ok()).is_some_and(|n| menu().contains(Some(&n)));
        if !inside {
            close_menu();
        }
    });
    listen(&doc(), "keydown", |e| {
        if e.unchecked_ref::<KeyboardEvent>().key() == "Escape" && !menu().hidden() {
            close_menu();
        }
    });
    let win = web_sys::window().unwrap();
    listen(&win, "resize", |_| {
        close_menu();
        fit();
    });
    chrome::on_bookmarks_changed(|| spawn(render()));
    spawn(render());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, ft: Option<&str>, kids: Vec<BookmarkNode>) -> BookmarkNode {
        BookmarkNode {
            id: id.into(),
            title: id.into(),
            url: if ft.is_none() && kids.is_empty() { Some(format!("https://{id}.com/")) } else { None },
            folder_type: ft.map(String::from),
            children: Some(kids),
            ..Default::default()
        }
    }

    #[test]
    fn finds_local_and_account_bars() {
        let leaf = |id: &str| node(id, None, vec![]);
        let old = vec![node("0", None, vec![node("1", None, vec![leaf("a")]), node("2", None, vec![leaf("b")])])];
        assert_eq!(bar_items(&old).iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["a"]);

        let synced = vec![node(
            "0",
            None,
            vec![
                node("1", Some("bookmarks-bar"), vec![leaf("local")]),
                node("2", Some("other"), vec![leaf("o")]),
                node("100", Some("bookmarks-bar"), vec![leaf("acct1"), leaf("acct2")]),
            ],
        )];
        assert_eq!(bar_items(&synced).iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["acct1", "acct2", "local"]);
    }
}
