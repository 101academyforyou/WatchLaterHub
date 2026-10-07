//! 拖曳排序的共用小工具（收藏、TODO、書籤共用）
//!
//! 用法：`sortable(&item, "todo", id, false, on_drop)`。
//! 拖曳中的項目記在 thread_local，只有同一組（group）的項目之間可以互拖。

use crate::ui::listen;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, Element, HtmlElement};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pos {
    Before,
    After,
    /// 放進資料夾裡（只有 `can_contain` 的項目）
    Into,
}

pub type OnDrop = Rc<dyn Fn(String, String, Pos)>;

thread_local! {
    /// (組別, 被拖曳的 id, 被拖曳的元素)
    static DRAGGING: RefCell<Option<(&'static str, String, Element)>> = const { RefCell::new(None) };
}

const MARKS: [&str; 3] = ["drop-before", "drop-after", "drop-into"];

fn clear_marks(e: &Element) {
    for m in MARKS {
        let _ = e.class_list().remove_1(m);
    }
}

/// 滑鼠在元素上半部 → 前面；下半部 → 後面；可放入的資料夾中間 1/2 → 放進去
/// 元素有 `data-drag-x` 屬性時（橫向排列，例如頂列書籤列）改看左右
fn pos_of(target: &Element, ev: &DragEvent, can_contain: bool) -> Pos {
    let r = target.get_bounding_client_rect();
    let y = if target.has_attribute("data-drag-x") {
        (ev.client_x() as f64 - r.left()) / r.width().max(1.0)
    } else {
        (ev.client_y() as f64 - r.top()) / r.height().max(1.0)
    };
    if can_contain {
        if y < 0.25 {
            Pos::Before
        } else if y > 0.75 {
            Pos::After
        } else {
            Pos::Into
        }
    } else if y < 0.5 {
        Pos::Before
    } else {
        Pos::After
    }
}

fn dragging_in(group: &str) -> Option<(String, Element)> {
    DRAGGING.with(|d| d.borrow().as_ref().filter(|(g, _, _)| *g == group).map(|(_, id, el)| (id.clone(), el.clone())))
}

pub struct Sortable {
    /// 代表這個項目的元素（拖曳時半透明；資料夾則用來判斷「不能拖進自己裡面」）
    pub item: Element,
    /// 按住這裡可以拖曳；None 表示只能放、不能拖（例如書籤列本身）
    pub handle: Option<Element>,
    /// 放下的判定範圍
    pub zone: Element,
    pub group: &'static str,
    pub id: String,
    pub can_contain: bool,
}

/// 讓項目可以拖曳、也可以接受同組項目放到它前面／後面／裡面
pub fn sortable(s: Sortable, on_drop: OnDrop) {
    let Sortable { item, handle, zone, group, id, can_contain } = s;
    let zone = &zone;
    if let Some(handle) = &handle {
        let _ = handle.set_attribute("draggable", "true");
        let (id, me) = (id.clone(), item.clone());
        listen(handle, "dragstart", move |e| {
            e.stop_propagation(); // 巢狀資料夾：只拖最內層
            let ev: &DragEvent = e.unchecked_ref();
            if let Some(dt) = ev.data_transfer() {
                dt.set_effect_allowed("move");
                let _ = dt.set_data("text/x-watchlaterhub", &id);
            }
            DRAGGING.with(|d| *d.borrow_mut() = Some((group, id.clone(), me.clone())));
            let _ = me.class_list().add_1("dragging");
        });
        let me = item.clone();
        listen(handle, "dragend", move |_| {
            let _ = me.class_list().remove_1("dragging");
            DRAGGING.with(|d| *d.borrow_mut() = None);
            // 清掉所有殘留的提示線
            if let Ok(list) = crate::ui::doc().query_selector_all(".drop-before, .drop-after, .drop-into") {
                for i in 0..list.length() {
                    if let Some(n) = list.item(i) {
                        clear_marks(n.unchecked_ref());
                    }
                }
            }
        });
    }
    {
        let (id, me) = (id.clone(), zone.clone());
        listen(zone, "dragover", move |e| {
            let Some((from, from_el)) = dragging_in(group) else { return };
            // 不能拖到自己，也不能把資料夾拖進自己的子資料夾
            if from == id || from_el.contains(Some(me.unchecked_ref())) {
                return;
            }
            e.prevent_default();
            e.stop_propagation();
            let ev: &DragEvent = e.unchecked_ref();
            if let Some(dt) = ev.data_transfer() {
                dt.set_drop_effect("move");
            }
            let p = pos_of(&me, ev, can_contain);
            clear_marks(&me);
            let _ = me.class_list().add_1(match p {
                Pos::Before => "drop-before",
                Pos::After => "drop-after",
                Pos::Into => "drop-into",
            });
        });
    }
    {
        let me = zone.clone();
        listen(zone, "dragleave", move |e| {
            // 移到子元素時也會觸發 dragleave，只在真的離開時清除
            let ev: &DragEvent = e.unchecked_ref();
            let still_inside = ev
                .related_target()
                .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                .map(|n| me.contains(Some(&n)))
                .unwrap_or(false);
            if !still_inside {
                clear_marks(&me);
            }
        });
    }
    {
        let me = zone.clone();
        listen(zone, "drop", move |e| {
            let Some((from, from_el)) = dragging_in(group) else { return };
            e.prevent_default();
            e.stop_propagation();
            clear_marks(&me);
            if from == id || from_el.contains(Some(me.unchecked_ref())) {
                return;
            }
            let p = pos_of(&me, e.unchecked_ref(), can_contain);
            on_drop(from, id.clone(), p);
        });
    }
}

/// 拖曳手把（⋮⋮），滑鼠移上去才出現
pub fn grip() -> HtmlElement {
    let s: HtmlElement = crate::ui::doc().create_element("span").unwrap().unchecked_into();
    s.set_class_name("grip");
    s.set_title("拖曳調整順序");
    s.set_text_content(Some("⋮⋮"));
    let _ = s.set_attribute("aria-hidden", "true");
    s
}
