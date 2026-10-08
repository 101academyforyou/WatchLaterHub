//! 影片左邊「這部影片在講什麼」：用 Chrome 內建 AI（Summarizer API，Gemini Nano）整理目前影片的重點
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
    /// 這次開新分頁自動整理失敗過的影片（不再自動重試，改顯示按鈕）
    static FAILED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// 上次因為要下載「影片語言 → 英文」翻譯模型而停下的語言；下次按按鈕時立刻開始下載
    static NEED_INPUT_LANG: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// 整理過的重點存在 chrome.storage，再看到同一部影片就不用重算
/// （v2：舊版把中文內容直接丟給 AI，整理出的亂碼不再沿用）
const CACHE_KEY: &str = "aiSummaries2";
const CACHE_MAX: usize = 30;
/// 字幕或說明少於這個字數就不整理（內容太少，AI 只會亂猜）
const MIN_INPUT_CHARS: usize = 80;
/// 要先翻成英文、才能下載模型時的錯誤（顯示「下載翻譯模型」按鈕）
const NEED_DOWNLOAD: &str = "這部影片不是英文，第一次要先下載翻譯模型";
/// 內容太少：按「重試」也沒用，不顯示按鈕
const TOO_SHORT: &str = "這部影片沒有字幕，說明也太短，無法整理重點";
/// 送給 AI 的文字上限：只送前面約 3,000 字，整理得比較快（還會依模型的 inputQuota 再縮短）
const MAX_INPUT_CHARS: usize = 3_000;

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
        return status("AI 整理中…");
    }
    let en = crate::i18n::is_en();
    let hit = cache().await.into_iter().find(|c| c.id == v.id && c.en == en);
    show_points(hit.as_ref());
    status("");
    if hit.is_some() {
        return hide("vai-first", true);
    }
    hide("vai-go", true); // 確認模型狀態前先不顯示按鈕，避免自動整理時閃一下
    // 模型已經下載好：不用按，直接整理
    let ready = models_ready(en).await;
    if !current().is_some_and(|c| c.id == v.id) {
        return; // 等待時換了影片，交給新的 render
    }
    let failed = FAILED.with(|f| f.borrow().contains(&v.id));
    if ready && !failed {
        hide("vai-first", true);
        return Box::pin(summarize()).await;
    }
    // 第一次：要使用者按一下才能下載模型（Chrome 的規定）
    text("vai-go", if ready { "用 AI 整理重點" } else { "下載 AI 模型（只需第一次）" });
    hide("vai-go", false);
    hide("vai-first", ready);
}

/// 需要的 AI 模型都已經在這台電腦上（不用下載、不用使用者按按鈕就能用）
async fn models_ready(en: bool) -> bool {
    availability("Summarizer", json!({ "type": "key-points", "format": "plain-text", "length": "short", "outputLanguage": "en" })).await == "available"
        && (en || availability("Translator", json!({ "sourceLanguage": "en", "targetLanguage": "zh-Hant" })).await == "available")
}

fn show_points(c: Option<&Cached>) {
    let ul = el("vai-points");
    ul.set_inner_html("");
    hide("vai-go", c.is_some());
    hide("vai-points", c.is_none());
    let Some(c) = c else { return };
    let doc = crate::ui::doc();
    for p in &c.points {
        let li = doc.create_element("li").unwrap();
        li.set_text_content(Some(p));
        ul.append_child(&li).unwrap();
    }
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
        Some(create_translator("en", "zh-Hant"))
    };
    // 上次停在「要下載影片語言的翻譯模型」：趁使用者剛按了按鈕，立刻開始下載
    let input_translator = NEED_INPUT_LANG.with(|n| n.borrow_mut().take()).map(|lang| (lang.clone(), create_translator(&lang, "en")));

    let res = run(&v, summarizer, input_translator).await;
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
            if here() {
                hide("vai-first", true);
            }
        }
        Err(e) => {
            chrome::warn(&format!("WatchLaterHub AI summary failed: {e}"));
            FAILED.with(|f| f.borrow_mut().push(v.id.clone()));
            if here() {
                text("vai-go", if e == NEED_DOWNLOAD { "下載翻譯模型（只需第一次）" } else { "用 AI 整理重點" });
                hide("vai-go", e == TOO_SHORT);
                status(&if e == TOO_SHORT { e } else { format!("⚠ {e}") });
            }
        }
    }
    BUSY.with(|b| b.borrow_mut().retain(|id| *id != v.id));
}

/// 讀影片 → 產生英文重點
async fn run(v: &Video, summarizer: Result<JsValue, String>, input_translator: Option<(String, Result<JsValue, String>)>) -> Result<Cached, String> {
    let status = |msg: &str| {
        if current().is_some_and(|c| c.id == v.id) {
            status(msg);
        }
    };
    status("讀取影片資訊…");
    let (body, captions) = video_text(&v.id).await;
    if body.trim().chars().count() < MIN_INPUT_CHARS {
        return Err(TOO_SHORT.into());
    }
    // 內建 AI 只看得懂英文（和西、日文）：中文等其他語言的內容先在電腦上翻成英文，
    // 不然會整理出亂七八糟的句子
    let mut title = format!("\"{}\" by {}", v.title, v.author);
    let mut body = truncate_chars(&body, MAX_INPUT_CHARS).to_string();
    let body_lang = script_lang(&body);
    if body_lang.is_none() && script_lang(&title).is_some() {
        // 內容是英文、標題不是：標題不翻譯，也不給 AI 看（避免整理出夾雜的文字）
        title.clear();
    }
    if let Some(lang) = body_lang {
        status("翻譯影片內容…");
        let t = match input_translator.filter(|(l, _)| l == lang) {
            Some((_, t)) => t,
            None => match availability("Translator", json!({ "sourceLanguage": lang, "targetLanguage": "en" })).await.as_str() {
                "available" => create_translator(lang, "en"),
                "" | "unavailable" => return Err("這部影片的語言目前無法用 AI 整理".into()),
                _ if user_active() => create_translator(lang, "en"),
                _ => {
                    NEED_INPUT_LANG.with(|n| *n.borrow_mut() = Some(lang.to_string()));
                    return Err(NEED_DOWNLOAD.into());
                }
            },
        };
        let t = await_js(t?).await?;
        let tr = |s: String| {
            let t = t.clone();
            async move { await_js(invoke(&t, "translate", &[JsValue::from_str(&s)])?).await?.as_string().ok_or_else(|| "翻譯失敗".to_string()) }
        };
        if !title.is_empty() {
            title = tr(title).await?;
        }
        body = tr(body).await?;
        let _ = invoke(&t, "destroy", &[]);
    }
    status("AI 整理中…");
    let s = await_js(summarizer?).await?;
    let input = fit_input(&s, &body).await;
    let context = if title.is_empty() { "A YouTube video.".to_string() } else { format!("A YouTube video: {title}.") };
    let opts = to_js(&json!({ "context": context }));
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
    if let Some((url, lang)) = pick_track(&player) {
        // 不是英文字幕：先請 YouTube 自動翻成英文，拿不到再用原文（之後在電腦上翻）
        let mut urls = vec![format!("{url}&fmt=json3")];
        if !lang.starts_with("en") {
            urls.insert(0, format!("{url}&fmt=json3&tlang=en"));
        }
        for u in urls {
            if let Ok((200, body)) = chrome::fetch_text(&u, "GET", None, &[]).await {
                transcript = serde_json::from_str::<Value>(&body).map(|j| transcript_from_json3(&j)).unwrap_or_default();
            }
            if !transcript.is_empty() {
                break;
            }
        }
    }
    let captions = !transcript.is_empty();
    // 已有英文字幕時，其他語言的說明就不放（避免中英混雜）
    let desc = if captions && script_lang(&desc).is_some() && script_lang(&transcript).is_none() { String::new() } else { desc };
    (build_input(&desc, &transcript), captions)
}

/// 選字幕：手動上傳的優先，其次英文，最後任一條（含自動產生的）。回傳 (網址, 語言)
pub fn pick_track(player: &Value) -> Option<(String, String)> {
    let tracks = player["captions"]["playerCaptionsTracklistRenderer"]["captionTracks"].as_array()?;
    let manual = |t: &&Value| t["kind"].as_str() != Some("asr");
    let english = |t: &&Value| t["languageCode"].as_str().is_some_and(|l| l.starts_with("en"));
    tracks
        .iter()
        .find(|t| manual(t) && english(t))
        .or_else(|| tracks.iter().find(manual))
        .or_else(|| tracks.first())
        .and_then(|t| Some((t["baseUrl"].as_str()?.to_string(), t["languageCode"].as_str().unwrap_or("").to_string())))
}

/// 內容主要是哪種非拉丁文字：日文（有假名）、韓文、中文；英文等拉丁文字回傳 None
pub fn script_lang(s: &str) -> Option<&'static str> {
    let (mut latin, mut han, mut kana, mut hangul) = (0usize, 0usize, 0usize, 0usize);
    for c in s.chars() {
        match c {
            '\u{3040}'..='\u{30ff}' => kana += 1,
            '\u{ac00}'..='\u{d7af}' | '\u{1100}'..='\u{11ff}' => hangul += 1,
            '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' => han += 1,
            c if c.is_alphabetic() => latin += 1,
            _ => {}
        }
    }
    // 中日韓一個字約等於英文一個詞（約 5 個字母），所以乘 5 比較
    let cjk = han + kana + hangul;
    if cjk * 5 < latin {
        return None;
    }
    Some(if kana * 10 > cjk { "ja" } else if hangul > han { "ko" } else { "zh" })
}

/// 使用者剛按過按鈕（Chrome 規定下載模型要在這之後）
fn user_active() -> bool {
    let nav = Reflect::get(&js_sys::global(), &"navigator".into()).unwrap_or(JsValue::UNDEFINED);
    Reflect::get(&nav, &"userActivation".into())
        .and_then(|u| Reflect::get(&u, &"isActive".into()))
        .ok()
        .and_then(|a| a.as_bool())
        .unwrap_or(false)
}

fn create_translator(from: &str, to: &str) -> Result<JsValue, String> {
    invoke_api("Translator", "create", &[with_monitor(json!({ "sourceLanguage": from, "targetLanguage": to }))])
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
        (false, false) => format!("Description:\n{}\n\nTranscript:\n{}", truncate_chars(desc.trim(), 500), transcript.trim()),
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
    let a = availability("Summarizer", json!({ "type": "key-points", "format": "plain-text", "length": "short", "outputLanguage": "en" })).await;
    !a.is_empty() && a != "unavailable"
}

/// "available"／"downloadable"／"downloading"／"unavailable"；不支援時是空字串
async fn availability(name: &str, opts: Value) -> String {
    match invoke_api(name, "availability", &[to_js(&opts)]) {
        Ok(p) => await_js(p).await.ok().and_then(|a| a.as_string()).unwrap_or_default(),
        Err(_) => String::new(),
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
            // 只在整理目前這部影片時顯示（避免蓋掉其他訊息）
            if current().is_some_and(|v| BUSY.with(|b| b.borrow().contains(&v.id))) {
                status(&format!("下載 AI 模型中… {}%", (loaded * 100.0).round() as i64));
            }
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
        assert_eq!(pick_track(&p), Some(("en".into(), "en-US".into())));
        let p = json!({ "captions": { "playerCaptionsTracklistRenderer": { "captionTracks": [
            { "baseUrl": "asr-en", "languageCode": "en", "kind": "asr" },
        ]}}});
        assert_eq!(pick_track(&p), Some(("asr-en".into(), "en".into())));
        assert_eq!(pick_track(&json!({})), None);
    }

    #[test]
    fn languages() {
        assert_eq!(script_lang("Learn Rust today, with examples"), None);
        assert_eq!(script_lang("Python 開發環境介紹 Colab 免費 GPU"), Some("zh"));
        assert_eq!(script_lang("今日はPythonの環境を紹介します"), Some("ja"));
        assert_eq!(script_lang("오늘은 파이썬을 소개합니다"), Some("ko"));
        // 英文為主、夾一兩個中文字
        assert_eq!(script_lang("This talk covers DSMGA-II and genetic algorithms in depth (基因)"), None);
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
