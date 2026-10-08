//! 左側「這部影片在講什麼」：用 Chrome 內建 AI（Summarizer API，Gemini Nano）整理目前影片的重點
//!
//! 影片的字幕與說明從 YouTube 影片頁讀取，重點在使用者自己的電腦上產生，不會傳到其他伺服器。
//! 中文模式先產生英文重點，再用 Chrome 內建的 Translator API 翻成繁體中文（翻不了就顯示英文）。
//! 瀏覽器不支援內建 AI 時，整個面板不顯示。

use crate::chrome::{self, to_js};
use crate::playlist::extract_json_after;
use crate::ui::{el, hide, on_click, spawn, text};
use crate::videos::Video;
use js_sys::{Array, Function, Promise, Reflect};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

thread_local! {
    static CURRENT: RefCell<Option<Video>> = const { RefCell::new(None) };
    /// 這個瀏覽器能不能用內建 AI（啟動時檢查一次）
    static AVAILABLE: Cell<bool> = const { Cell::new(false) };
    /// 正在整理的影片 ID
    static BUSY: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// 整理過的重點存在 chrome.storage，再看到同一部影片就不用重算
const CACHE_KEY: &str = "aiSummaries";
const CACHE_MAX: usize = 30;
/// 送給 AI 的文字上限（還會依模型的 inputQuota 再縮短）
const MAX_INPUT_CHARS: usize = 12_000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Cached {
    id: String,
    en: bool,
    points: Vec<String>,
    /// 有沒有讀到字幕（沒有就是依影片說明整理）
    captions: bool,
}

pub fn start() {
    on_click("vai-go", || spawn(summarize()));
    spawn(async {
        let ok = summarizer_available().await;
        AVAILABLE.with(|a| a.set(ok));
        render().await;
    });
}

/// 隨機影片換了（None = 沒有影片）
pub fn set_current(v: Option<Video>) {
    CURRENT.with(|c| *c.borrow_mut() = v);
    spawn(render());
}

fn current() -> Option<Video> {
    CURRENT.with(|c| c.borrow().clone())
}

fn status(msg: &str) {
    text("vai-status", msg);
    hide("vai-status", msg.is_empty());
}

async fn render() {
    let v = current();
    hide("vai", !AVAILABLE.with(|a| a.get()) || v.is_none());
    let Some(v) = v else { return };
    if BUSY.with(|b| b.borrow().contains(&v.id)) {
        hide("vai-go", true);
        hide("vai-points", true);
        hide("vai-src", true);
        return status("AI 整理中…");
    }
    let en = crate::i18n::is_en();
    let hit = cache().await.into_iter().find(|c| c.id == v.id && c.en == en);
    show_points(hit.as_ref());
    status("");
}

fn show_points(c: Option<&Cached>) {
    let ul = el("vai-points");
    ul.set_inner_html("");
    hide("vai-go", c.is_some());
    hide("vai-points", c.is_none());
    hide("vai-src", c.is_none());
    let Some(c) = c else { return };
    let doc = crate::ui::doc();
    for p in &c.points {
        let li = doc.create_element("li").unwrap();
        li.set_text_content(Some(p));
        ul.append_child(&li).unwrap();
    }
    text("vai-src", if c.captions { "依字幕整理" } else { "依影片說明整理（沒有字幕）" });
}

async fn summarize() {
    let Some(v) = current() else { return };
    if BUSY.with(|b| b.borrow().contains(&v.id)) {
        return;
    }
    BUSY.with(|b| b.borrow_mut().push(v.id.clone()));
    hide("vai-go", true);
    let en = crate::i18n::is_en();
    // 先建立 AI（模型還沒下載時需要使用者剛按過按鈕），再去讀影片
    let summarizer = invoke_api("Summarizer", "create", &[with_monitor(json!({
        "type": "key-points", "format": "plain-text", "length": "short", "outputLanguage": "en",
    }))]);
    let translator = if en {
        None
    } else {
        Some(invoke_api("Translator", "create", &[with_monitor(json!({ "sourceLanguage": "en", "targetLanguage": "zh-Hant" }))]))
    };

    let res = run(&v, summarizer).await;
    let here = || current().is_some_and(|c| c.id == v.id);
    match res {
        Ok(mut c) => {
            // 先顯示英文重點，翻譯好再換成中文（翻譯模型第一次要下載，可能要等一陣子）
            if here() {
                show_points(Some(&c));
                status("");
            }
            if let Some(Ok(t)) = translator {
                if here() {
                    status("翻譯中…");
                }
                if let Some(zh) = translate_all(t, &c.points).await {
                    c = Cached { en: false, points: zh, ..c };
                }
                if here() {
                    show_points(Some(&c));
                    status("");
                }
            }
            // 翻譯失敗時存成英文，下次在中文模式還能再試翻譯
            save(&c).await;
        }
        Err(e) => {
            chrome::warn(&format!("WatchLaterHub AI summary failed: {e}"));
            if here() {
                hide("vai-go", false);
                status(&format!("⚠ {e}"));
            }
        }
    }
    BUSY.with(|b| b.borrow_mut().retain(|id| *id != v.id));
}

/// 讀影片 → 產生英文重點
async fn run(v: &Video, summarizer: Result<JsValue, String>) -> Result<Cached, String> {
    let status = |msg: &str| {
        if current().is_some_and(|c| c.id == v.id) {
            status(msg);
        }
    };
    status("讀取影片資訊…");
    let (body, captions) = video_text(&v.id).await;
    if body.trim().is_empty() {
        return Err("這部影片沒有字幕或說明，無法整理重點".into());
    }
    status("AI 整理中…");
    let s = await_js(summarizer?).await?;
    let input = fit_input(&s, &body).await;
    let opts = to_js(&json!({ "context": format!("A YouTube video titled \"{}\" by {}.", v.title, v.author) }));
    let out = await_js(invoke(&s, "summarize", &[JsValue::from_str(&input), opts])?).await;
    let _ = invoke(&s, "destroy", &[]);
    let points = parse_points(&out?.as_string().unwrap_or_default());
    if points.is_empty() {
        return Err("AI 沒有產生內容".into());
    }
    Ok(Cached { id: v.id.clone(), en: true, points, captions })
}

/// 一點一點翻成繁體中文；任何一點失敗就回傳 None（維持英文）
async fn translate_all(translator: JsValue, points: &[String]) -> Option<Vec<String>> {
    let t = await_js(translator).await.ok()?;
    let mut zh = Vec::new();
    for p in points {
        let r = invoke(&t, "translate", &[JsValue::from_str(p)]).ok()?;
        zh.push(await_js(r).await.ok()?.as_string()?);
    }
    let _ = invoke(&t, "destroy", &[]);
    Some(zh)
}

/// 依模型的 inputQuota 縮短輸入
async fn fit_input(s: &JsValue, body: &str) -> String {
    let mut input = truncate_chars(body, MAX_INPUT_CHARS).to_string();
    let quota = Reflect::get(s, &"inputQuota".into()).ok().and_then(|q| q.as_f64()).unwrap_or(f64::INFINITY);
    for _ in 0..4 {
        let used = match invoke(s, "measureInputUsage", &[JsValue::from_str(&input)]) {
            Ok(p) => await_js(p).await.ok().and_then(|u| u.as_f64()),
            Err(_) => None,
        };
        let Some(used) = used.filter(|u| *u > quota) else { break };
        let keep = (input.chars().count() as f64 * quota / used * 0.9) as usize;
        input = truncate_chars(&input, keep).to_string();
    }
    input
}

// ---------- 影片的字幕與說明 ----------

/// 回傳 (要整理的文字, 是否有字幕)
async fn video_text(id: &str) -> (String, bool) {
    let page = format!("https://www.youtube.com/watch?v={id}&hl=en");
    let Ok((200, html)) = chrome::fetch_text(&page, "GET", None, &[]).await else { return (String::new(), false) };
    let Some(player) = extract_json_after(&html, "ytInitialPlayerResponse = ") else { return (String::new(), false) };
    let desc = clean_description(player["videoDetails"]["shortDescription"].as_str().unwrap_or(""));
    let mut transcript = String::new();
    if let Some(url) = pick_track(&player) {
        if let Ok((200, body)) = chrome::fetch_text(&format!("{url}&fmt=json3"), "GET", None, &[]).await {
            transcript = serde_json::from_str::<Value>(&body).map(|j| transcript_from_json3(&j)).unwrap_or_default();
        }
    }
    let captions = !transcript.is_empty();
    (build_input(&desc, &transcript), captions)
}

/// 選字幕：手動上傳的優先，其次英文，最後任一條（含自動產生的）
pub fn pick_track(player: &Value) -> Option<String> {
    let tracks = player["captions"]["playerCaptionsTracklistRenderer"]["captionTracks"].as_array()?;
    let manual = |t: &&Value| t["kind"].as_str() != Some("asr");
    let english = |t: &&Value| t["languageCode"].as_str().is_some_and(|l| l.starts_with("en"));
    tracks
        .iter()
        .find(|t| manual(t) && english(t))
        .or_else(|| tracks.iter().find(manual))
        .or_else(|| tracks.first())
        .and_then(|t| t["baseUrl"].as_str())
        .map(String::from)
}

/// YouTube 字幕（fmt=json3）→ 一整段文字
pub fn transcript_from_json3(j: &Value) -> String {
    let mut out = String::new();
    for ev in j["events"].as_array().into_iter().flatten() {
        for seg in ev["segs"].as_array().into_iter().flatten() {
            if let Some(t) = seg["utf8"].as_str() {
                out.push_str(t);
                out.push(' ');
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 影片說明：拿掉網址與只剩符號的行（通常是贊助、社群連結）
pub fn clean_description(d: &str) -> String {
    d.lines()
        .map(|l| l.split_whitespace().filter(|w| !w.contains("://") && !w.starts_with("www.")).collect::<Vec<_>>().join(" "))
        .filter(|l| l.chars().any(|c| c.is_alphanumeric()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn build_input(desc: &str, transcript: &str) -> String {
    match (desc.trim().is_empty(), transcript.trim().is_empty()) {
        (_, true) => desc.trim().to_string(),
        (true, false) => transcript.trim().to_string(),
        (false, false) => format!("Description:\n{}\n\nTranscript:\n{}", truncate_chars(desc.trim(), 1500), transcript.trim()),
    }
}

/// AI 回的重點（每行一點，可能帶 * - • 1. 或 **粗體**）→ 一點一項
pub fn parse_points(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| {
            let l = l.trim().trim_start_matches(['*', '-', '•', '·']).trim_start();
            let l = match l.split_once(". ") {
                Some((n, rest)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => rest,
                _ => l,
            };
            l.replace("**", "").trim().to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn truncate_chars(s: &str, n: usize) -> &str {
    s.char_indices().nth(n).map(|(i, _)| &s[..i]).unwrap_or(s)
}

// ---------- 快取 ----------

async fn cache() -> Vec<Cached> {
    chrome::get_or(CACHE_KEY, Vec::new()).await
}

async fn save(c: &Cached) {
    let list = remember(cache().await, c.clone());
    chrome::set(&[(CACHE_KEY, to_js(&list))]).await;
}

/// 新的放最後，同一部影片同一語言只留一份，最多 CACHE_MAX 份
fn remember(mut list: Vec<Cached>, c: Cached) -> Vec<Cached> {
    list.retain(|x| !(x.id == c.id && x.en == c.en));
    list.push(c);
    let extra = list.len().saturating_sub(CACHE_MAX);
    list.drain(..extra);
    list
}

// ---------- Chrome 內建 AI ----------

/// 全域物件（Summarizer、Translator）；瀏覽器不支援時是 None
fn api(name: &str) -> Option<JsValue> {
    Reflect::get(&js_sys::global(), &name.into()).ok().filter(|v| !v.is_undefined() && !v.is_null())
}

async fn summarizer_available() -> bool {
    let Some(s) = api("Summarizer") else { return false };
    let opts = to_js(&json!({ "type": "key-points", "format": "plain-text", "length": "short", "outputLanguage": "en" }));
    match invoke(&s, "availability", &[opts]) {
        Ok(p) => await_js(p).await.ok().and_then(|a| a.as_string()).is_some_and(|a| a != "unavailable"),
        Err(_) => false,
    }
}

/// 呼叫 `obj.method(...args)`（同步呼叫，回傳值若是 Promise 交給 await_js）
fn invoke(obj: &JsValue, method: &str, args: &[JsValue]) -> Result<JsValue, String> {
    let f: Function = Reflect::get(obj, &method.into())
        .map_err(chrome::err_msg)?
        .dyn_into()
        .map_err(|_| format!("瀏覽器不支援 {method}"))?;
    f.apply(obj, &args.iter().collect::<Array>()).map_err(chrome::err_msg)
}

fn invoke_api(name: &str, method: &str, args: &[JsValue]) -> Result<JsValue, String> {
    let obj = api(name).ok_or_else(|| format!("瀏覽器不支援 {name}"))?;
    invoke(&obj, method, args)
}

async fn await_js(v: JsValue) -> Result<JsValue, String> {
    match v.dyn_into::<Promise>() {
        Ok(p) => JsFuture::from(p).await.map_err(chrome::err_msg),
        Err(v) => Ok(v),
    }
}

/// 建立選項加上 monitor：第一次使用要下載模型，顯示進度
fn with_monitor(opts: Value) -> JsValue {
    let o = to_js(&opts);
    let monitor = Closure::<dyn FnMut(JsValue)>::new(|m: JsValue| {
        let on_progress = Closure::<dyn FnMut(JsValue)>::new(|e: JsValue| {
            let loaded = Reflect::get(&e, &"loaded".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
            status(&format!("下載 AI 模型中… {}%", (loaded * 100.0).round() as i64));
        });
        let _ = invoke(&m, "addEventListener", &[JsValue::from_str("downloadprogress"), on_progress.into_js_value()]);
    });
    let _ = Reflect::set(&o, &"monitor".into(), &monitor.into_js_value());
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points() {
        assert_eq!(parse_points("* First point\n\n- **Second** point\n• third\n2. fourth\n"), vec!["First point", "Second point", "third", "fourth"]);
        assert_eq!(parse_points("2.5 GHz is fast"), vec!["2.5 GHz is fast"]);
        assert!(parse_points("  \n").is_empty());
    }

    #[test]
    fn tracks() {
        let p = json!({ "captions": { "playerCaptionsTracklistRenderer": { "captionTracks": [
            { "baseUrl": "asr-en", "languageCode": "en", "kind": "asr" },
            { "baseUrl": "zh", "languageCode": "zh-TW" },
            { "baseUrl": "en", "languageCode": "en-US" },
        ]}}});
        assert_eq!(pick_track(&p).as_deref(), Some("en"));
        let p = json!({ "captions": { "playerCaptionsTracklistRenderer": { "captionTracks": [
            { "baseUrl": "asr-en", "languageCode": "en", "kind": "asr" },
        ]}}});
        assert_eq!(pick_track(&p).as_deref(), Some("asr-en"));
        assert_eq!(pick_track(&json!({})), None);
    }

    #[test]
    fn json3() {
        let j = json!({ "events": [
            { "tStartMs": 0 },
            { "segs": [{ "utf8": "hello" }, { "utf8": " world" }] },
            { "segs": [{ "utf8": "\n" }] },
            { "segs": [{ "utf8": "again" }] },
        ]});
        assert_eq!(transcript_from_json3(&j), "hello world again");
    }

    #[test]
    fn description() {
        let d = "Learn Rust today.\nhttps://example.com\nFollow me: https://x.com/me\n---\nChapters";
        assert_eq!(clean_description(d), "Learn Rust today.\nFollow me:\nChapters");
    }

    #[test]
    fn input() {
        assert_eq!(build_input("desc", ""), "desc");
        assert_eq!(build_input("", "words"), "words");
        assert_eq!(build_input("desc", "words"), "Description:\ndesc\n\nTranscript:\nwords");
        assert_eq!(truncate_chars("中文字幕", 2), "中文");
        assert_eq!(truncate_chars("ab", 5), "ab");
    }

    #[test]
    fn cache_limit() {
        let c = |id: &str, en| Cached { id: id.into(), en, points: vec![], captions: false };
        let mut list = vec![];
        for i in 0..CACHE_MAX + 5 {
            list = remember(list, c(&i.to_string(), false));
        }
        assert_eq!(list.len(), CACHE_MAX);
        assert_eq!(list[0].id, "5");
        let list = remember(list, c("5", false));
        assert_eq!(list.len(), CACHE_MAX);
        assert_eq!(list.last().unwrap().id, "5");
        let list = remember(list, c("5", true));
        assert_eq!(list.iter().filter(|x| x.id == "5").count(), 2);
    }
}
