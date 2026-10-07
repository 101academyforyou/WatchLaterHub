//! Chrome 擴充功能 API 與瀏覽器全域函式的 Rust 綁定

use js_sys::{Object, Reflect, JSON};
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    // chrome.storage.local
    #[wasm_bindgen(js_namespace = ["chrome", "storage", "local"], js_name = get, catch)]
    async fn storage_get(keys: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "storage", "local"], js_name = set, catch)]
    async fn storage_set(items: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "storage", "local"], js_name = remove, catch)]
    async fn storage_remove(keys: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "storage", "onChanged"], js_name = addListener)]
    pub fn on_storage_changed(cb: &Closure<dyn FnMut(JsValue, JsValue)>);

    // chrome.identity
    #[wasm_bindgen(js_namespace = ["chrome", "identity"], js_name = launchWebAuthFlow, catch)]
    async fn launch_web_auth_flow_raw(details: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "identity"], js_name = getRedirectURL)]
    pub fn redirect_url() -> String;

    // chrome.contextMenus / chrome.alarms
    #[wasm_bindgen(js_namespace = ["chrome", "contextMenus"], js_name = create)]
    fn context_menus_create_raw(props: JsValue);
    #[wasm_bindgen(js_namespace = ["chrome", "alarms"], js_name = create)]
    fn alarms_create_raw(name: &str, info: JsValue);
    #[wasm_bindgen(js_namespace = ["chrome", "alarms"], js_name = clear, catch)]
    async fn alarms_clear_raw(name: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "alarms"], js_name = getAll, catch)]
    async fn alarms_get_all_raw() -> Result<JsValue, JsValue>;

    // chrome.history
    #[wasm_bindgen(js_namespace = ["chrome", "history"], js_name = search, catch)]
    async fn history_search_raw(query: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "history"], js_name = getVisits, catch)]
    async fn history_get_visits_raw(details: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "history", "onVisited"], js_name = addListener)]
    fn history_on_visited(cb: &js_sys::Function);

    // chrome.windows（提醒小視窗）
    #[wasm_bindgen(js_namespace = ["chrome", "windows"], js_name = create, catch)]
    async fn windows_create_raw(props: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "windows"], js_name = getLastFocused, catch)]
    async fn windows_get_last_focused_raw() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "windows"], js_name = update, catch)]
    async fn windows_update_raw(id: f64, props: JsValue) -> Result<JsValue, JsValue>;

    // chrome.bookmarks
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks"], js_name = getTree, catch)]
    async fn bookmarks_get_tree_raw() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks"], js_name = get, catch)]
    async fn bookmarks_get_raw(id: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks"], js_name = search, catch)]
    async fn bookmarks_search_raw(query: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks"], js_name = create, catch)]
    async fn bookmarks_create_raw(bookmark: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks"], js_name = update, catch)]
    async fn bookmarks_update_raw(id: &str, changes: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks"], js_name = "move", catch)]
    async fn bookmarks_move_raw(id: &str, dest: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks"], js_name = remove, catch)]
    async fn bookmarks_remove_raw(id: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks", "onCreated"], js_name = addListener)]
    fn bm_on_created(cb: &js_sys::Function);
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks", "onRemoved"], js_name = addListener)]
    fn bm_on_removed(cb: &js_sys::Function);
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks", "onChanged"], js_name = addListener)]
    fn bm_on_changed(cb: &js_sys::Function);
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks", "onMoved"], js_name = addListener)]
    fn bm_on_moved(cb: &js_sys::Function);
    #[wasm_bindgen(js_namespace = ["chrome", "bookmarks", "onChildrenReordered"], js_name = addListener)]
    fn bm_on_reordered(cb: &js_sys::Function);

    // chrome.tabs.update：開啟 chrome:// 等一般連結打不開的網址
    #[wasm_bindgen(js_namespace = ["chrome", "tabs"], js_name = update, catch)]
    async fn tabs_update_raw(tab_id: f64, props: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "tabs"], js_name = getCurrent, catch)]
    async fn tabs_get_current_raw() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "tabs"], js_name = create, catch)]
    async fn tabs_create_raw(props: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = getURL)]
    fn runtime_get_url(path: &str) -> String;

    // navigator.geolocation（manifest 要有 "geolocation" 權限）
    #[wasm_bindgen(js_namespace = ["navigator", "geolocation"], js_name = getCurrentPosition)]
    fn geo_get_current(success: &js_sys::Function, error: &js_sys::Function, opts: JsValue);

    // chrome.cookies（manifest 要有 "cookies" 權限）
    #[wasm_bindgen(js_namespace = ["chrome", "cookies"], js_name = get, catch)]
    async fn cookies_get_raw(details: JsValue) -> Result<JsValue, JsValue>;

    // 全域 fetch（新分頁與 service worker 都能用）
    #[wasm_bindgen(js_name = fetch)]
    fn fetch_raw(url: &str, init: JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["navigator", "clipboard"], js_name = writeText)]
    fn clipboard_write_raw(text: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    pub fn warn(s: &str);
}

pub type R<T> = Result<T, String>;

/// 把 JS 錯誤轉成文字訊息
pub fn err_msg(e: JsValue) -> String {
    Reflect::get(&e, &"message".into())
        .ok()
        .and_then(|m| m.as_string())
        .or_else(|| e.as_string())
        .unwrap_or_else(|| "未知錯誤".into())
}

pub fn to_js<T: Serialize + ?Sized>(v: &T) -> JsValue {
    serde_json::to_string(v).ok().and_then(|s| JSON::parse(&s).ok()).unwrap_or(JsValue::NULL)
}

pub fn from_js<T: DeserializeOwned>(v: &JsValue) -> Option<T> {
    if v.is_undefined() || v.is_null() {
        return None;
    }
    let s: String = JSON::stringify(v).ok()?.into();
    serde_json::from_str(&s).ok()
}

pub fn now() -> f64 {
    js_sys::Date::now()
}

// ---------- storage ----------

pub async fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
    let res = storage_get(JsValue::from_str(key)).await.ok()?;
    from_js(&Reflect::get(&res, &key.into()).ok()?)
}

pub async fn get_or<T: DeserializeOwned>(key: &str, default: T) -> T {
    get(key).await.unwrap_or(default)
}

/// 一次寫入多個鍵：`set(&[("a", to_js(&1)), ...])`
pub async fn set(items: &[(&str, JsValue)]) {
    let o = Object::new();
    for (k, v) in items {
        let _ = Reflect::set(&o, &(*k).into(), v);
    }
    if let Err(e) = storage_set(o.into()).await {
        warn(&format!("storage.set failed: {}", err_msg(e)));
    }
}

pub async fn remove(keys: &[&str]) {
    let arr = js_sys::Array::new();
    for k in keys {
        arr.push(&(*k).into());
    }
    let _ = storage_remove(arr.into()).await;
}

// ---------- identity ----------

pub async fn launch_web_auth_flow(url: &str, interactive: bool) -> R<String> {
    let d = Object::new();
    let _ = Reflect::set(&d, &"url".into(), &url.into());
    let _ = Reflect::set(&d, &"interactive".into(), &interactive.into());
    let r = launch_web_auth_flow_raw(d.into()).await.map_err(err_msg)?;
    r.as_string().ok_or_else(|| "登入沒有回傳結果".into())
}

// ---------- menus / alarms ----------

pub fn context_menu(id: &str, title: &str, contexts: &str, pattern_key: &str, patterns: &[&str]) {
    context_menus_create_raw(to_js(&serde_json::json!({
        "id": id, "title": title, "contexts": [contexts], pattern_key: patterns
    })));
}

/// 單次鬧鐘：在 `when`（毫秒時間戳）響
pub fn alarm_at(name: &str, when: f64) {
    alarms_create_raw(name, to_js(&serde_json::json!({ "when": when })));
}

pub async fn alarm_clear(name: &str) {
    let _ = alarms_clear_raw(name).await;
}

pub async fn alarm_names() -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct A {
        name: String,
    }
    let v = alarms_get_all_raw().await.ok();
    v.and_then(|v| from_js::<Vec<A>>(&v)).unwrap_or_default().into_iter().map(|a| a.name).collect()
}

// ---------- history ----------

#[derive(serde::Deserialize, Clone, Debug)]
pub struct HistoryItem {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "lastVisitTime", default)]
    pub last_visit: f64,
}

/// `since`（毫秒時間戳）之後瀏覽過的網頁，最多 `max` 筆
pub async fn history_since(since: f64, max: u32) -> Vec<HistoryItem> {
    let q = serde_json::json!({ "text": "", "startTime": since, "maxResults": max });
    match history_search_raw(to_js(&q)).await {
        Ok(v) => from_js(&v).unwrap_or_default(),
        Err(_) => vec![],
    }
}

/// `start`～`end`（毫秒時間戳）之間瀏覽過的網頁
pub async fn history_range(start: f64, end: f64, max: u32) -> Vec<HistoryItem> {
    let q = serde_json::json!({ "text": "", "startTime": start, "endTime": end, "maxResults": max });
    match history_search_raw(to_js(&q)).await {
        Ok(v) => from_js(&v).unwrap_or_default(),
        Err(_) => vec![],
    }
}

/// 某個網址每一次的瀏覽時間（毫秒時間戳）
pub async fn history_visit_times(url: &str) -> Vec<f64> {
    #[derive(serde::Deserialize)]
    struct V {
        #[serde(rename = "visitTime", default)]
        t: f64,
    }
    match history_get_visits_raw(to_js(&serde_json::json!({ "url": url }))).await {
        Ok(v) => from_js::<Vec<V>>(&v).unwrap_or_default().into_iter().map(|x| x.t).collect(),
        Err(_) => vec![],
    }
}

pub fn on_history_visited(f: impl FnMut() + 'static) {
    let cb = Closure::<dyn FnMut()>::new(f);
    history_on_visited(cb.as_ref().unchecked_ref());
    cb.forget();
}

// ---------- windows ----------

/// 跳出一個獨立的小視窗（置中在目前的 Chrome 視窗上方），並讓它取得焦點
pub async fn open_popup_window(path: &str, width: i32, height: i32) {
    let mut props = serde_json::json!({
        "url": runtime_get_url(path), "type": "popup", "focused": true, "width": width, "height": height
    });
    if let Ok(w) = windows_get_last_focused_raw().await {
        let num = |k: &str| Reflect::get(&w, &k.into()).ok().and_then(|v| v.as_f64());
        if let (Some(l), Some(t), Some(ww)) = (num("left"), num("top"), num("width")) {
            props["left"] = ((l + (ww - width as f64) / 2.0).max(0.0) as i32).into();
            props["top"] = ((t + 120.0).max(0.0) as i32).into();
        }
    }
    // 算出的位置若超出螢幕（多螢幕、視窗比螢幕大…）Chrome 會拒絕，改用預設位置再開一次
    let win = match windows_create_raw(to_js(&props)).await {
        Ok(w) => Ok(w),
        Err(_) => {
            if let Some(o) = props.as_object_mut() {
                o.remove("left");
                o.remove("top");
            }
            windows_create_raw(to_js(&props)).await
        }
    };
    match win {
        Ok(win) => {
            // 工作列／Dock 也閃一下提醒
            if let Some(id) = Reflect::get(&win, &"id".into()).ok().and_then(|v| v.as_f64()) {
                let _ = windows_update_raw(id, to_js(&serde_json::json!({ "focused": true, "drawAttention": true }))).await;
            }
        }
        Err(e) => warn(&format!("windows.create failed: {}", err_msg(e))),
    }
}

/// 開新分頁到擴充功能自己的頁面（背景也能用）
pub async fn open_extension_page(path: &str) {
    open_in_new_tab(&runtime_get_url(path), true).await;
}

// ---------- bookmarks ----------

/// chrome.bookmarks 的節點：有 `url` 是書籤，有 `children` 是資料夾
#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct BookmarkNode {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(rename = "parentId", default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub index: Option<u32>,
    #[serde(default)]
    pub children: Option<Vec<BookmarkNode>>,
    /// "bookmarks-bar"、"other"、"mobile"（Chrome 134 起才有；登入同步書籤時書籤列的 id 不一定是 "1"）
    #[serde(rename = "folderType", default)]
    pub folder_type: Option<String>,
}

pub async fn bookmarks_tree() -> R<Vec<BookmarkNode>> {
    let v = bookmarks_get_tree_raw().await.map_err(err_msg)?;
    Ok(from_js(&v).unwrap_or_default())
}

/// 依 id 取得一個書籤或資料夾
pub async fn bookmark_get(id: &str) -> Option<BookmarkNode> {
    let v = bookmarks_get_raw(id).await.ok()?;
    from_js::<Vec<BookmarkNode>>(&v)?.into_iter().next()
}

/// 網址完全相同的書籤
pub async fn bookmarks_find_url(url: &str) -> Vec<BookmarkNode> {
    match bookmarks_search_raw(to_js(&serde_json::json!({ "url": url }))).await {
        Ok(v) => from_js(&v).unwrap_or_default(),
        Err(_) => vec![],
    }
}

/// 依名稱或網址搜尋書籤（只回傳書籤，不含資料夾）
pub async fn bookmarks_search(query: &str) -> Vec<BookmarkNode> {
    match bookmarks_search_raw(JsValue::from_str(query)).await {
        Ok(v) => from_js::<Vec<BookmarkNode>>(&v).unwrap_or_default().into_iter().filter(|n| n.url.is_some()).collect(),
        Err(_) => vec![],
    }
}

pub async fn bookmarks_create(parent_id: &str, title: &str, url: &str, index: Option<u32>) -> R<()> {
    let mut b = serde_json::json!({ "parentId": parent_id, "title": title, "url": url });
    if let Some(i) = index {
        b["index"] = i.into();
    }
    bookmarks_create_raw(to_js(&b))
        .await
        .map(|_| ())
        .map_err(err_msg)
}

/// 建立資料夾，回傳新資料夾的 id
pub async fn bookmarks_create_folder(parent_id: &str, title: &str) -> R<String> {
    let v = bookmarks_create_raw(to_js(&serde_json::json!({ "parentId": parent_id, "title": title })))
        .await
        .map_err(err_msg)?;
    Reflect::get(&v, &"id".into()).ok().and_then(|x| x.as_string()).ok_or_else(|| "建立資料夾沒有回傳 id".into())
}

/// 修改名稱與網址，並搬到 `parent_id` 資料夾
pub async fn bookmarks_edit(id: &str, title: &str, url: &str, parent_id: &str) -> R<()> {
    bookmarks_update_raw(id, to_js(&serde_json::json!({ "title": title, "url": url })))
        .await
        .map_err(err_msg)?;
    // 資料夾沒變就不搬，保留原本的位置
    if bookmark_get(id).await.and_then(|n| n.parent_id).as_deref() == Some(parent_id) {
        return Ok(());
    }
    bookmarks_move_raw(id, to_js(&serde_json::json!({ "parentId": parent_id })))
        .await
        .map(|_| ())
        .map_err(err_msg)
}

/// 搬到 `parent_id` 資料夾的第 `index` 個位置（None = 最後）
pub async fn bookmarks_move(id: &str, parent_id: &str, index: Option<u32>) -> R<()> {
    let mut dest = serde_json::json!({ "parentId": parent_id });
    if let Some(i) = index {
        dest["index"] = i.into();
    }
    bookmarks_move_raw(id, to_js(&dest)).await.map(|_| ()).map_err(err_msg)
}

pub async fn bookmarks_remove(id: &str) -> R<()> {
    bookmarks_remove_raw(id).await.map(|_| ()).map_err(err_msg)
}

/// 書籤有任何新增、刪除、改名、搬移時呼叫 `f`
pub fn on_bookmarks_changed(f: impl FnMut() + 'static) {
    let cb = Closure::<dyn FnMut()>::new(f);
    let func: &js_sys::Function = cb.as_ref().unchecked_ref();
    bm_on_created(func);
    bm_on_removed(func);
    bm_on_changed(func);
    bm_on_moved(func);
    bm_on_reordered(func);
    cb.forget();
}

/// 這個新分頁自己的 (分頁 id, 位置)
async fn current_tab() -> Option<(f64, f64)> {
    let t = tabs_get_current_raw().await.ok()?;
    let id = Reflect::get(&t, &"id".into()).ok()?.as_f64()?;
    let index = Reflect::get(&t, &"index".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    Some((id, index))
}

/// 在這個分頁開啟（chrome:// 等網址一般連結打不開，所以用 chrome.tabs）
pub async fn open_in_this_tab(url: &str) {
    let ok = match current_tab().await {
        Some((id, _)) => tabs_update_raw(id, to_js(&serde_json::json!({ "url": url }))).await.is_ok(),
        None => false,
    };
    if !ok {
        // 退而求其次：直接跳轉（一般 http/https 網址可行）
        let _ = web_sys::window().unwrap().location().set_href(url);
    }
}

/// 在目前分頁旁邊開新分頁
pub async fn open_in_new_tab(url: &str, active: bool) {
    let mut props = serde_json::json!({ "url": url, "active": active });
    if let Some((id, index)) = current_tab().await {
        props["openerTabId"] = id.into();
        props["index"] = (index + 1.0).into();
    }
    if let Err(e) = tabs_create_raw(to_js(&props)).await {
        warn(&format!("tabs.create failed: {}", err_msg(e)));
    }
}

/// 網站小圖示（需要 manifest 的 "favicon" 權限）
pub fn favicon_url(page_url: &str) -> String {
    format!("{}?pageUrl={}&size=32", runtime_get_url("/_favicon/"), js_sys::encode_uri_component(page_url))
}

// ---------- 定位 ----------

/// 目前位置 (緯度, 經度)；被拒絕、逾時或系統沒開定位就回傳 None
pub async fn current_position() -> Option<(f64, f64)> {
    let nav = Reflect::get(&js_sys::global(), &"navigator".into()).unwrap_or(JsValue::UNDEFINED);
    let has_geo = !nav.is_undefined() && Reflect::has(&nav, &"geolocation".into()).unwrap_or(false);
    if !has_geo {
        return None;
    }
    let p = js_sys::Promise::new(&mut |resolve, _reject| {
        let ok = resolve.clone();
        let success = Closure::once_into_js(move |pos: JsValue| {
            let _ = ok.call1(&JsValue::NULL, &pos);
        });
        let fail = Closure::once_into_js(move |_e: JsValue| {
            let _ = resolve.call1(&JsValue::NULL, &JsValue::NULL);
        });
        let opts = to_js(&serde_json::json!({ "timeout": 8000, "maximumAge": 1_800_000 }));
        geo_get_current(success.unchecked_ref(), fail.unchecked_ref(), opts);
    });
    let pos = JsFuture::from(p).await.ok()?;
    if pos.is_null() {
        return None;
    }
    let c = Reflect::get(&pos, &"coords".into()).ok()?;
    let lat = Reflect::get(&c, &"latitude".into()).ok()?.as_f64()?;
    let lon = Reflect::get(&c, &"longitude".into()).ok()?.as_f64()?;
    Some((lat, lon))
}

// ---------- fetch ----------

pub struct Resp {
    pub status: u16,
    pub json: serde_json::Value,
}

pub async fn fetch(url: &str, bearer: Option<&str>, method: &str) -> R<Resp> {
    let mut init = serde_json::json!({ "method": method });
    if let Some(t) = bearer {
        init["headers"] = serde_json::json!({ "Authorization": format!("Bearer {t}") });
    }
    let r = JsFuture::from(fetch_raw(url, to_js(&init))).await.map_err(err_msg)?;
    let r: web_sys::Response = r.dyn_into().map_err(|_| "fetch 回傳格式錯誤".to_string())?;
    let status = r.status();
    let json = match r.json() {
        Ok(p) => JsFuture::from(p).await.ok().and_then(|v| from_js(&v)).unwrap_or(serde_json::Value::Null),
        Err(_) => serde_json::Value::Null,
    };
    Ok(Resp { status, json })
}

/// 回傳 (HTTP 狀態碼, 內文文字)。帶上 youtube.com 的 cookie，讓「不公開」清單也讀得到
/// `headers`：額外的 HTTP 標頭（例如讀取「稍後觀看」時的登入驗證）
pub async fn fetch_text(url: &str, method: &str, json_body: Option<&str>, headers: &[(String, String)]) -> R<(u16, String)> {
    let mut init = serde_json::json!({ "method": method, "credentials": "include" });
    let mut h = serde_json::Map::new();
    if let Some(b) = json_body {
        init["body"] = b.into();
        h.insert("Content-Type".into(), "application/json".into());
    }
    for (k, v) in headers {
        h.insert(k.clone(), v.clone().into());
    }
    if !h.is_empty() {
        init["headers"] = h.into();
    }
    let r = JsFuture::from(fetch_raw(url, to_js(&init))).await.map_err(err_msg)?;
    let r: web_sys::Response = r.dyn_into().map_err(|_| "fetch 回傳格式錯誤".to_string())?;
    let status = r.status();
    let text = match r.text() {
        Ok(p) => JsFuture::from(p).await.ok().and_then(|v| v.as_string()).unwrap_or_default(),
        Err(_) => String::new(),
    };
    Ok((status, text))
}

/// 讀取某網址的 cookie 值（沒有或沒權限時回傳 None）
pub async fn cookie(url: &str, name: &str) -> Option<String> {
    let v = cookies_get_raw(to_js(&serde_json::json!({ "url": url, "name": name }))).await.ok()?;
    from_js::<serde_json::Value>(&v)?["value"].as_str().map(String::from)
}

pub async fn clipboard_write(text: &str) {
    let _ = JsFuture::from(clipboard_write_raw(text)).await;
}
