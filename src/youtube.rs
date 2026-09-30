//! Google 登入（帳號選擇畫面）與 YouTube Data API 同步

use crate::chrome::{self, to_js, R};
use crate::config;
use crate::store::YT_KEY;
use crate::videos::{dedupe, is_valid_client_id, parse_fragment, query, Video};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const LIKED: &str = "LIKED"; // 代表「喜歡的影片」的來源 ID
const API: &str = "https://www.googleapis.com/youtube/v3/";
const USERINFO: &str = "https://www.googleapis.com/oauth2/v3/";
const SCOPES: &str = "https://www.googleapis.com/auth/youtube.readonly email";
pub const AUTH_LOST: &str = "授權已失效，請重新登入";
pub const CANCELLED: &str = "已取消登入";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Source {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Channel {
    pub title: String,
    #[serde(default)]
    pub thumb: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Token {
    token: String,
    exp: f64,
}

pub struct Settings {
    pub connected: bool,
    pub channel: Option<Channel>,
    pub sources: Vec<String>,
    pub synced_at: f64,
    pub error: String,
}

pub async fn settings() -> Settings {
    Settings {
        connected: chrome::get_or("ytConnected", false).await,
        channel: chrome::get("ytChannel").await,
        sources: chrome::get_or("ytSources", vec![LIKED.to_string()]).await,
        synced_at: chrome::get_or("ytSyncedAt", 0.0).await,
        error: chrome::get_or("ytError", String::new()).await,
    }
}

// ---------- Client ID ----------

/// 來源優先順序：config.rs（發佈給使用者時用）> 設定精靈存的值
pub async fn client_id() -> Option<String> {
    if is_valid_client_id(config::GOOGLE_CLIENT_ID) {
        return Some(config::GOOGLE_CLIENT_ID.to_string());
    }
    chrome::get::<String>("ytClientId").await.filter(|s| is_valid_client_id(s))
}

pub async fn is_configured() -> bool {
    client_id().await.is_some()
}

pub async fn set_client_id(id: &str) -> R<()> {
    let id = id.trim();
    if !is_valid_client_id(id) {
        return Err("格式不對，應該長得像 1234-abc.apps.googleusercontent.com".into());
    }
    chrome::set(&[("ytClientId", to_js(id))]).await;
    Ok(())
}

// ---------- OAuth ----------

/// interactive=true：顯示 Google 帳號選擇畫面；false：背景靜默換新 token（不跳視窗）
async fn get_token(interactive: bool) -> R<String> {
    if let Some(t) = chrome::get::<Token>("ytToken").await {
        if t.exp - 60_000.0 > chrome::now() {
            return Ok(t.token);
        }
    }
    let cid = client_id().await.ok_or("尚未設定 Client ID")?;
    let email: String = chrome::get_or("ytEmail", String::new()).await;
    let redirect = chrome::redirect_url();
    let mut params = vec![
        ("client_id", cid.as_str()),
        ("response_type", "token"),
        ("redirect_uri", redirect.as_str()),
        ("scope", SCOPES),
        ("include_granted_scopes", "true"),
        ("prompt", if interactive { "select_account" } else { "none" }),
    ];
    if !email.is_empty() {
        params.push(("login_hint", email.as_str()));
    }
    let url = format!("https://accounts.google.com/o/oauth2/v2/auth?{}", query(&params));

    let back = match chrome::launch_web_auth_flow(&url, interactive).await {
        Ok(b) => b,
        Err(_) if !interactive => return Err(AUTH_LOST.into()),
        Err(e) => return Err(e),
    };
    let frag = parse_fragment(&back);
    let field = |k: &str| frag.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
    match field("error").as_deref() {
        Some("access_denied") => return Err(CANCELLED.into()),
        Some(e) if interactive => return Err(format!("登入失敗：{e}")),
        Some(_) => return Err(AUTH_LOST.into()),
        None => {}
    }
    let token = field("access_token").ok_or("登入沒有取得 access token")?;
    let secs: f64 = field("expires_in").and_then(|s| s.parse().ok()).unwrap_or(3600.0);
    let t = Token { token: token.clone(), exp: chrome::now() + secs * 1000.0 };
    chrome::set(&[("ytToken", to_js(&t))]).await;
    Ok(token)
}

/// 呼叫 Google API；token 失效時靜默換新重試一次
async fn api(base: &str, path: &str, params: &[(&str, &str)], interactive: bool) -> R<Value> {
    let url = format!("{base}{path}?{}", query(params));
    let mut token = get_token(interactive).await?;
    for attempt in 0..2 {
        let r = chrome::fetch(&url, Some(&token), "GET").await?;
        if r.status == 401 && attempt == 0 {
            chrome::remove(&["ytToken"]).await;
            token = get_token(false).await?;
            continue;
        }
        if r.status >= 400 {
            return Err(r.json["error"]["message"]
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| format!("YouTube API 錯誤 {}", r.status)));
        }
        return Ok(r.json);
    }
    Err(AUTH_LOST.into())
}

/// 翻頁抓取所有項目
async fn pages(path: &str, params: &[(&str, &str)], map: fn(&Value) -> Option<Video>) -> R<Vec<Video>> {
    let mut out = vec![];
    let mut page_token = String::new();
    loop {
        let mut p: Vec<(&str, &str)> = params.to_vec();
        p.push(("maxResults", "50"));
        if !page_token.is_empty() {
            p.push(("pageToken", &page_token));
        }
        let j = api(API, path, &p, false).await?;
        if let Some(items) = j["items"].as_array() {
            out.extend(items.iter().filter_map(map));
        }
        match j["nextPageToken"].as_str() {
            Some(t) if out.len() < config::MAX_PER_SOURCE => page_token = t.to_string(),
            _ => break,
        }
    }
    out.truncate(config::MAX_PER_SOURCE);
    Ok(out)
}

pub async fn list_sources() -> R<Vec<Source>> {
    let mut out = vec![Source { id: LIKED.into(), title: "👍 喜歡的影片".into(), count: None }];
    let mut page_token = String::new();
    loop {
        let mut p = vec![("part", "snippet,contentDetails"), ("mine", "true"), ("maxResults", "50")];
        if !page_token.is_empty() {
            p.push(("pageToken", &page_token));
        }
        let j = api(API, "playlists", &p, false).await?;
        for pl in j["items"].as_array().into_iter().flatten() {
            out.push(Source {
                id: pl["id"].as_str().unwrap_or_default().into(),
                title: pl["snippet"]["title"].as_str().unwrap_or_default().into(),
                count: pl["contentDetails"]["itemCount"].as_u64(),
            });
        }
        match j["nextPageToken"].as_str() {
            Some(t) => page_token = t.to_string(),
            None => break,
        }
    }
    Ok(out)
}

fn liked_item(v: &Value) -> Option<Video> {
    Some(Video::new(v["id"].as_str()?, v["snippet"]["title"].as_str()?, v["snippet"]["channelTitle"].as_str().unwrap_or("")))
}

fn playlist_item(it: &Value) -> Option<Video> {
    let s = &it["snippet"];
    let owner = s["videoOwnerChannelTitle"].as_str()?; // 已刪除或私人影片沒有這個欄位
    Some(Video::new(s["resourceId"]["videoId"].as_str()?, s["title"].as_str()?, owner))
}

async fn fetch_source(id: &str) -> R<Vec<Video>> {
    if id == LIKED {
        pages("videos", &[("part", "snippet"), ("myRating", "like")], liked_item).await
    } else {
        pages("playlistItems", &[("part", "snippet"), ("playlistId", id)], playlist_item).await
    }
}

/// 一鍵登入：選帳號授權 → 取得頻道資訊與清單 → 預設全選 → 同步
pub async fn connect() -> R<usize> {
    chrome::remove(&["ytToken"]).await;
    get_token(true).await?;
    let me = api(USERINFO, "userinfo", &[], false).await?;
    chrome::set(&[("ytEmail", to_js(me["email"].as_str().unwrap_or("")))]).await;

    let ch = api(API, "channels", &[("part", "snippet"), ("mine", "true")], false).await?;
    let channel = ch["items"][0]["snippet"]["title"].as_str().map(|title| Channel {
        title: title.into(),
        thumb: ch["items"][0]["snippet"]["thumbnails"]["default"]["url"].as_str().map(String::from),
    });
    let srcs = list_sources().await?;
    let ids: Vec<&str> = srcs.iter().map(|s| s.id.as_str()).collect();
    chrome::set(&[
        ("ytConnected", to_js(&true)),
        ("ytChannel", to_js(&channel)),
        ("ytSourceList", to_js(&srcs)),
        ("ytSources", to_js(&ids)),
        ("ytSyncStart", to_js(&0)),
    ])
    .await;
    sync().await
}

/// 同步所有勾選的來源到本機快取，回傳影片數
pub async fn sync() -> R<usize> {
    let s = settings().await;
    if !s.connected || !is_configured().await {
        return Ok(0);
    }
    // 避免同時開多個分頁時重複同步
    if chrome::now() - chrome::get_or("ytSyncStart", 0.0).await < 120_000.0 {
        return Ok(0);
    }
    chrome::set(&[("ytSyncStart", to_js(&chrome::now()))]).await;

    let mut all = vec![];
    let mut result = Ok(());
    for src in &s.sources {
        match fetch_source(src).await {
            Ok(v) => all.extend(v),
            Err(e) => {
                result = Err(e);
                break;
            }
        }
    }
    let out = match result {
        Ok(()) => {
            let list = dedupe(all);
            let n = list.len();
            chrome::set(&[(YT_KEY, to_js(&list)), ("ytSyncedAt", to_js(&chrome::now())), ("ytError", to_js(""))]).await;
            Ok(n)
        }
        Err(e) => {
            // 授權失效時切回「未登入」狀態（保留已同步的影片），讓頁面重新顯示登入按鈕
            let mut items = vec![("ytError", to_js(&e))];
            if e == AUTH_LOST {
                items.push(("ytConnected", to_js(&false)));
            }
            chrome::set(&items).await;
            Err(e)
        }
    };
    chrome::set(&[("ytSyncStart", to_js(&0))]).await;
    out
}

pub async fn set_sources(ids: &[String]) {
    chrome::set(&[("ytSources", to_js(ids))]).await;
}

pub async fn disconnect() {
    if let Some(t) = chrome::get::<Token>("ytToken").await {
        let url = format!("https://oauth2.googleapis.com/revoke?{}", query(&[("token", &t.token)]));
        let _ = chrome::fetch(&url, None, "POST").await;
    }
    chrome::remove(&["ytToken", "ytEmail"]).await;
    let empty: Vec<Video> = vec![];
    let no_src: Vec<Source> = vec![];
    chrome::set(&[
        ("ytConnected", to_js(&false)),
        ("ytChannel", wasm_bindgen::JsValue::NULL),
        ("ytSourceList", to_js(&no_src)),
        (YT_KEY, to_js(&empty)),
        ("ytSyncedAt", to_js(&0)),
        ("ytError", to_js("")),
    ])
    .await;
}
