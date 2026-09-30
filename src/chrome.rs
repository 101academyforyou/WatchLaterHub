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

pub fn alarm(name: &str, period_min: f64) {
    alarms_create_raw(name, to_js(&serde_json::json!({ "periodInMinutes": period_min })));
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
pub async fn fetch_text(url: &str, method: &str, json_body: Option<&str>) -> R<(u16, String)> {
    let mut init = serde_json::json!({ "method": method, "credentials": "include" });
    if let Some(b) = json_body {
        init["body"] = b.into();
        init["headers"] = serde_json::json!({ "Content-Type": "application/json" });
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

pub async fn clipboard_write(text: &str) {
    let _ = JsFuture::from(clipboard_write_raw(text)).await;
}
