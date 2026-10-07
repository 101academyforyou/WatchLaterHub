//! 播放清單 → 所有影片連結（不需要登入，也不耗用 YouTube API 配額）
//!
//! 做法跟瀏覽器打開播放清單頁面一樣：
//! 1. 讀 `youtube.com/playlist?list=…` 頁面裡的 `ytInitialData`（前 100 部）
//! 2. 用頁面給的 continuation token 呼叫 `youtubei/v1/browse` 繼續往下翻

use crate::chrome::{self, R};
use crate::videos::{dedupe, Video};
use serde_json::{json, Value};
use url::Url;

/// 最多翻幾頁（每頁約 100 部），避免 YouTube 改版時無限迴圈
const MAX_PAGES: usize = 100;

fn is_playlist_id(s: &str) -> bool {
    s.len() >= 12 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 從播放清單網址（或 watch?v=…&list=… 網址、或單純清單 ID）取出清單 ID
pub fn parse_playlist_id(text: &str) -> Option<String> {
    let text = text.trim();
    if let Ok(u) = Url::parse(text) {
        let host = u.host_str()?;
        if !(host.ends_with("youtube.com") || host == "youtu.be") {
            return None;
        }
        return u.query_pairs().find(|(k, _)| k == "list").map(|(_, v)| v.to_string()).filter(|v| is_playlist_id(v));
    }
    // 單純貼 ID（PL… / UU… / OLAK5uy_…），不接受 11 碼影片 ID
    (text.len() > 11 && is_playlist_id(text)).then(|| text.to_string())
}

/// 從 HTML 取出 `marker` 後面緊接的 JSON 物件（依大括號配對，會略過字串裡的括號）
pub fn extract_json_after(html: &str, marker: &str) -> Option<Value> {
    let start = html.find(marker)? + marker.len();
    let rest = &html[start..];
    let open = rest.find('{')?;
    let bytes = rest.as_bytes();
    let (mut depth, mut in_str, mut esc) = (0i32, false, false);
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        if in_str {
            match b {
                _ if esc => esc = false,
                b'\\' => esc = true,
                b'"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return serde_json::from_str(&rest[open..=i]).ok();
                }
            }
            _ => {}
        }
    }
    None
}

/// 從 HTML 取出 `"KEY":"value"` 的字串值（ytcfg 設定）
pub fn extract_cfg(html: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let start = html.find(&marker)? + marker.len();
    let end = html[start..].find('"')?;
    Some(html[start..start + end].to_string())
}

fn text_of(v: &Value) -> Option<String> {
    v["simpleText"]
        .as_str()
        .map(String::from)
        .or_else(|| v["runs"].as_array().map(|r| r.iter().filter_map(|x| x["text"].as_str()).collect()))
        .or_else(|| v["content"].as_str().map(String::from))
}

/// 走訪整個 JSON，收集影片與下一頁的 continuation token。
/// 同時支援舊版 `playlistVideoRenderer` 與新版 `lockupViewModel` 版面。
pub fn collect(v: &Value, videos: &mut Vec<Video>, next: &mut Option<String>) {
    match v {
        Value::Object(m) => {
            if let Some(r) = m.get("playlistVideoRenderer") {
                // 已刪除／私人影片沒有 shortBylineText，也不能播放，略過
                if let (Some(id), Some(author)) = (r["videoId"].as_str(), text_of(&r["shortBylineText"])) {
                    let title = text_of(&r["title"]).unwrap_or_else(|| id.to_string());
                    videos.push(Video::new(id, &title, &author));
                }
                return;
            }
            if let Some(r) = m.get("lockupViewModel") {
                if r["contentType"].as_str() == Some("LOCKUP_CONTENT_TYPE_VIDEO") {
                    if let Some(id) = r["contentId"].as_str() {
                        let meta = &r["metadata"]["lockupMetadataViewModel"];
                        let title = text_of(&meta["title"]).unwrap_or_else(|| id.to_string());
                        let author = meta["metadata"]["contentMetadataViewModel"]["metadataRows"][0]["metadataParts"][0]["text"]["content"]
                            .as_str()
                            .unwrap_or("");
                        videos.push(Video::new(id, &title, author));
                    }
                }
                return;
            }
            if let Some(t) = m.get("continuationCommand").and_then(|c| c["token"].as_str()) {
                next.get_or_insert_with(|| t.to_string());
            }
            for x in m.values() {
                collect(x, videos, next);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect(x, videos, next)),
        _ => {}
    }
}

/// 「稍後觀看」的清單 ID（只有登入 YouTube 的本人看得到）
pub const WATCH_LATER_ID: &str = "WL";

/// 抓取播放清單裡所有可播放的影片（依清單順序、去除重複）
pub async fn fetch_all(playlist_id: &str, progress: impl FnMut(usize)) -> R<Vec<Video>> {
    let list = fetch_list(playlist_id, progress).await?;
    if list.is_empty() {
        return Err("這個清單裡沒有可播放的影片（可能是私人清單，或影片都被刪除了）".into());
    }
    Ok(list)
}

/// 抓取「稍後觀看」：用這個瀏覽器目前登入的 YouTube 帳號讀取（YouTube API 不開放這個清單）
pub async fn fetch_watch_later() -> R<Vec<Video>> {
    fetch_list(WATCH_LATER_ID, |_| {}).await
}

/// 私人清單（稍後觀看）翻頁時要附上的登入驗證標頭；沒登入 YouTube 時回傳空的
async fn auth_headers(html: &str) -> Vec<(String, String)> {
    const ORIGIN: &str = "https://www.youtube.com";
    let mut h = vec![("X-Origin".to_string(), ORIGIN.to_string())];
    if let Some(i) = extract_cfg(html, "SESSION_INDEX") {
        h.push(("X-Goog-AuthUser".into(), i));
    }
    if let Some(v) = extract_cfg(html, "VISITOR_DATA") {
        h.push(("X-Goog-Visitor-Id".into(), v));
    }
    let sid = match chrome::cookie(ORIGIN, "SAPISID").await {
        Some(s) => Some(s),
        None => chrome::cookie(ORIGIN, "__Secure-3PAPISID").await,
    };
    if let Some(sid) = sid {
        let ts = (chrome::now() / 1000.0) as u64;
        h.push(("Authorization".into(), sapisid_hash(ts, &sid, ORIGIN)));
    }
    h
}

/// YouTube 網頁版的登入驗證：`SAPISIDHASH 時間_SHA1(時間 SAPISID 來源)`
pub fn sapisid_hash(ts: u64, sapisid: &str, origin: &str) -> String {
    format!("SAPISIDHASH {ts}_{}", sha1_hex(format!("{ts} {sapisid} {origin}").as_bytes()))
}

/// SHA-1（只用來產生上面的驗證字串）
pub fn sha1_hex(data: &[u8]) -> String {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

/// 讀取清單（可能是空的）
async fn fetch_list(playlist_id: &str, mut progress: impl FnMut(usize)) -> R<Vec<Video>> {
    let watch_later = playlist_id == WATCH_LATER_ID;
    let page = format!("https://www.youtube.com/playlist?list={playlist_id}&hl=zh-TW");
    let (status, html) = chrome::fetch_text(&page, "GET", None, &[]).await?;
    if status >= 400 {
        return Err(format!("打不開播放清單（HTTP {status}）"));
    }
    if watch_later && !html.contains("\"LOGGED_IN\":true") {
        return Err("讀不到「稍後觀看」：請先在這個 Chrome 登入 YouTube（youtube.com）".into());
    }
    let data = extract_json_after(&html, "ytInitialData = ")
        .or_else(|| extract_json_after(&html, "ytInitialData\"] = "))
        .ok_or("讀不到播放清單內容（YouTube 可能改版了）")?;
    if !watch_later && data["alerts"].to_string().contains("ERROR") && !data.to_string().contains("\"videoId\"") {
        return Err("這個播放清單不存在，或是「私人」清單（只有「公開」或「不公開」的清單能讀取）".into());
    }

    let mut videos = vec![];
    let mut next = None;
    collect(&data, &mut videos, &mut next);
    progress(videos.len());

    let version = extract_cfg(&html, "INNERTUBE_CLIENT_VERSION").unwrap_or_else(|| "2.20260901.00.00".into());
    let key = extract_cfg(&html, "INNERTUBE_API_KEY");
    let api = match &key {
        Some(k) => format!("https://www.youtube.com/youtubei/v1/browse?prettyPrint=false&key={k}"),
        None => "https://www.youtube.com/youtubei/v1/browse?prettyPrint=false".into(),
    };
    let headers = if watch_later { auth_headers(&html).await } else { vec![] };

    let mut seen_tokens = vec![];
    for _ in 0..MAX_PAGES {
        let Some(token) = next.take() else { break };
        if seen_tokens.contains(&token) {
            break;
        }
        seen_tokens.push(token.clone());
        let body = json!({
            "context": { "client": { "clientName": "WEB", "clientVersion": version, "hl": "zh-TW" } },
            "continuation": token,
        });
        let (status, text) = chrome::fetch_text(&api, "POST", Some(&body.to_string()), &headers).await?;
        if status >= 400 {
            break; // 已經拿到的先回傳
        }
        let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let before = videos.len();
        collect(&j, &mut videos, &mut next);
        progress(videos.len());
        if videos.len() == before {
            break;
        }
    }

    Ok(dedupe(videos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_playlist_ids() {
        let id = "PLFgquLnL59alCl_2TQvOiD5Vgm1hCaGSU";
        assert_eq!(parse_playlist_id(&format!("https://www.youtube.com/playlist?list={id}")).as_deref(), Some(id));
        assert_eq!(parse_playlist_id(&format!("https://m.youtube.com/watch?v=dQw4w9WgXcQ&list={id}&index=3")).as_deref(), Some(id));
        assert_eq!(parse_playlist_id(&format!("https://youtu.be/dQw4w9WgXcQ?list={id}")).as_deref(), Some(id));
        assert_eq!(parse_playlist_id(id).as_deref(), Some(id));
        assert_eq!(parse_playlist_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), None);
        assert_eq!(parse_playlist_id("dQw4w9WgXcQ"), None);
        assert_eq!(parse_playlist_id("https://example.com/?list=PLabcdefghijkl"), None);
    }

    #[test]
    fn sha1_and_sapisid_hash() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha1_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            sapisid_hash(1700000000, "abc", "https://www.youtube.com"),
            format!("SAPISIDHASH 1700000000_{}", sha1_hex(b"1700000000 abc https://www.youtube.com"))
        );
    }

    #[test]
    fn extracts_initial_data_with_braces_in_strings() {
        let html = r#"<script>var ytInitialData = {"a":"}{\"x","b":{"c":1}};</script><script>ytcfg.set({"INNERTUBE_CLIENT_VERSION":"2.20260930.01.00","INNERTUBE_API_KEY":"AIzaX"})"#;
        let v = extract_json_after(html, "ytInitialData = ").unwrap();
        assert_eq!(v["a"], "}{\"x");
        assert_eq!(v["b"]["c"], 1);
        assert_eq!(extract_cfg(html, "INNERTUBE_CLIENT_VERSION").as_deref(), Some("2.20260930.01.00"));
        assert_eq!(extract_cfg(html, "INNERTUBE_API_KEY").as_deref(), Some("AIzaX"));
    }

    #[test]
    fn collects_old_layout_and_continuation() {
        let data = json!({"contents": {"playlistVideoListRenderer": {"contents": [
            {"playlistVideoRenderer": {"videoId": "aaaaaaaaaaa", "title": {"runs": [{"text": "影片 A"}]}, "shortBylineText": {"runs": [{"text": "頻道 A"}]}}},
            {"playlistVideoRenderer": {"videoId": "bbbbbbbbbbb", "title": {"runs": [{"text": "[已刪除影片]"}]}}},
            {"continuationItemRenderer": {"continuationEndpoint": {"commandExecutorCommand": {"commands": [
                {"continuationCommand": {"token": "TOKEN1", "request": "CONTINUATION_REQUEST_TYPE_BROWSE"}}
            ]}}}}
        ]}}});
        let (mut v, mut next) = (vec![], None);
        collect(&data, &mut v, &mut next);
        assert_eq!(v, vec![Video::new("aaaaaaaaaaa", "影片 A", "頻道 A")]);
        assert_eq!(next.as_deref(), Some("TOKEN1"));
    }

    #[test]
    fn collects_new_lockup_layout() {
        let data = json!({"onResponseReceivedActions": [{"appendContinuationItemsAction": {"continuationItems": [
            {"lockupViewModel": {"contentId": "ccccccccccc", "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
              "metadata": {"lockupMetadataViewModel": {"title": {"content": "影片 C"},
                "metadata": {"contentMetadataViewModel": {"metadataRows": [{"metadataParts": [{"text": {"content": "頻道 C"}}]}]}}}}}},
            {"lockupViewModel": {"contentId": "PLxxxxxxxxxxxx", "contentType": "LOCKUP_CONTENT_TYPE_PLAYLIST"}}
        ]}}]});
        let (mut v, mut next) = (vec![], None);
        collect(&data, &mut v, &mut next);
        assert_eq!(v, vec![Video::new("ccccccccccc", "影片 C", "頻道 C")]);
        assert_eq!(next, None);
    }
}
