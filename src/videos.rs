//! 純邏輯（不依賴瀏覽器），可以用 `cargo test` 在本機測試

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Video {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(rename = "addedAt", default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<f64>,
}

impl Video {
    pub fn new(id: &str, title: &str, author: &str) -> Self {
        Video { id: id.into(), title: title.into(), author: author.into(), added_at: None }
    }
    pub fn watch_url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.id)
    }
    pub fn thumb(&self, size: &str) -> String {
        format!("https://i.ytimg.com/vi/{}/{}.jpg", self.id, size)
    }
}

/// 搜尋篩選：查詢字串以空白分成多個詞，每個詞都要出現在任一欄位裡（不分大小寫）；空白查詢一律符合
pub fn matches(query: &str, fields: &[&str]) -> bool {
    let hay: Vec<String> = fields.iter().map(|f| f.to_lowercase()).collect();
    query.to_lowercase().split_whitespace().all(|w| hay.iter().any(|h| h.contains(w)))
}

/// 拖曳排序：把 `from` 移到 `to` 的前面（`after` 為 true 時是後面）。找不到就不動，回傳是否有移動
pub fn move_by_key<T>(list: &mut Vec<T>, key: impl Fn(&T) -> &str, from: &str, to: &str, after: bool) -> bool {
    if from == to {
        return false;
    }
    let Some(i) = list.iter().position(|x| key(x) == from) else { return false };
    if !list.iter().any(|x| key(x) == to) {
        return false;
    }
    let item = list.remove(i);
    let j = list.iter().position(|x| key(x) == to).unwrap();
    list.insert(if after { j + 1 } else { j }, item);
    true
}

/// 使用者輸入的書籤網址：去掉空白，沒寫通訊協定就補 https://；不像網址則回傳 None
pub fn normalize_url(input: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() || s.contains(char::is_whitespace) {
        return None;
    }
    let has_scheme = s.split_once(':').is_some_and(|(sch, rest)| {
        !sch.is_empty()
            && sch.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
            && (rest.starts_with("//") || ["javascript", "about", "mailto", "data", "chrome"].contains(&sch.to_ascii_lowercase().as_str()))
    });
    let full = if has_scheme { s.to_string() } else { format!("https://{s}") };
    let u = Url::parse(&full).ok()?;
    if !has_scheme && !u.host_str().is_some_and(|h| h.contains('.') || h == "localhost") {
        return None;
    }
    Some(full)
}

fn is_video_id(s: &str) -> bool {
    s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 從各種 YouTube 網址（或單純 11 碼 ID）取出影片 ID
pub fn parse_id(text: &str) -> Option<String> {
    let text = text.trim();
    if is_video_id(text) {
        return Some(text.to_string());
    }
    let u = Url::parse(text).ok()?;
    let host = u.host_str()?;
    let host = ["www.", "m.", "music."]
        .iter()
        .find_map(|p| host.strip_prefix(p))
        .unwrap_or(host);

    let candidate: Option<String> = match host {
        "youtu.be" => u.path_segments()?.next().map(|s| s.chars().take(11).collect()),
        "youtube.com" | "youtube-nocookie.com" => {
            if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "v") {
                Some(v.chars().take(11).collect())
            } else {
                let mut seg = u.path_segments()?;
                match (seg.next(), seg.next()) {
                    (Some("shorts" | "embed" | "live" | "v"), Some(id)) => {
                        Some(id.chars().take(11).collect())
                    }
                    _ => None,
                }
            }
        }
        _ => None,
    };
    candidate.filter(|id| is_video_id(id))
}

/// 依 ID 去除重複，保留第一次出現的順序
pub fn dedupe(videos: impl IntoIterator<Item = Video>) -> Vec<Video> {
    let mut seen = HashSet::new();
    videos.into_iter().filter(|v| seen.insert(v.id.clone())).collect()
}

/// 隨機挑一部；有兩部以上時避開上一次那部。`r` 為 [0, 1) 的亂數
pub fn pick_random<'a>(list: &'a [Video], last_id: Option<&str>, r: f64) -> Option<&'a Video> {
    let pool: Vec<&Video> = if list.len() > 1 {
        list.iter().filter(|v| Some(v.id.as_str()) != last_id).collect()
    } else {
        list.iter().collect()
    };
    if pool.is_empty() {
        return None;
    }
    let i = ((r * pool.len() as f64) as usize).min(pool.len() - 1);
    Some(pool[i])
}

/// 使用者貼上的內容：以換行或逗號分隔
pub fn split_input(s: &str) -> Vec<String> {
    s.split(['\n', '\r', ','])
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(String::from)
        .collect()
}

pub fn is_valid_client_id(s: &str) -> bool {
    let s = s.trim();
    match s.strip_suffix(".apps.googleusercontent.com") {
        Some(p) => {
            !p.is_empty()
                && !s.starts_with("YOUR_")
                && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        }
        None => false,
    }
}

/// 「幾分鐘前」
pub fn ago(minutes: f64) -> String {
    let m = minutes.round() as i64;
    if m < 1 {
        "剛剛".into()
    } else if m < 60 {
        format!("{m} 分鐘前")
    } else if m < 1440 {
        format!("{} 小時前", (minutes / 60.0).round() as i64)
    } else {
        format!("{} 天前", (minutes / 1440.0).round() as i64)
    }
}

/// 解析 OAuth 重新導向網址的 #fragment
pub fn parse_fragment(redirect: &str) -> Vec<(String, String)> {
    Url::parse(redirect)
        .ok()
        .and_then(|u| u.fragment().map(|f| url::form_urlencoded::parse(f.as_bytes()).into_owned().collect()))
        .unwrap_or_default()
}

pub fn query(params: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new()).extend_pairs(params).finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "dQw4w9WgXcQ";

    #[test]
    fn parses_all_url_forms() {
        for u in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=10s",
            "https://youtu.be/dQw4w9WgXcQ?si=abc",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=x",
            "https://www.youtube.com/live/dQw4w9WgXcQ",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
            "  dQw4w9WgXcQ  ",
        ] {
            assert_eq!(parse_id(u).as_deref(), Some(ID), "{u}");
        }
        assert_eq!(parse_id("https://www.youtube.com/watch?v=CG1llQrJNbE&t=11s").as_deref(), Some("CG1llQrJNbE"));
        assert_eq!(parse_id("https://youtu.be/Tm_q8c4KzW0").as_deref(), Some("Tm_q8c4KzW0"));
    }

    #[test]
    fn rejects_non_videos() {
        for u in ["https://google.com", "not a url", "https://www.youtube.com/@channel", "https://youtu.be/", "abc"] {
            assert_eq!(parse_id(u), None, "{u}");
        }
    }

    #[test]
    fn dedupes_keeping_order() {
        let v = dedupe([Video::new("a", "1", ""), Video::new("b", "2", ""), Video::new("a", "3", "")]);
        assert_eq!(v.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), ["1", "2"]);
    }

    #[test]
    fn random_avoids_last() {
        let list = vec![Video::new("a", "", ""), Video::new("b", "", "")];
        for r in [0.0, 0.5, 0.999] {
            assert_eq!(pick_random(&list, Some("a"), r).unwrap().id, "b");
        }
        let one = vec![Video::new("a", "", "")];
        assert_eq!(pick_random(&one, Some("a"), 0.3).unwrap().id, "a");
        assert!(pick_random(&[], None, 0.3).is_none());
    }

    #[test]
    fn splits_input() {
        assert_eq!(split_input("a\r\n b ,c\n\n"), ["a", "b", "c"]);
        assert_eq!(split_input("not a url"), ["not a url"]);
    }

    #[test]
    fn validates_client_id() {
        assert!(is_valid_client_id("627463072769-ob84bkvfbeap56a9sga62a81cqb4ju5h.apps.googleusercontent.com"));
        assert!(!is_valid_client_id("YOUR_CLIENT_ID.apps.googleusercontent.com"));
        assert!(!is_valid_client_id("hello"));
        assert!(!is_valid_client_id(".apps.googleusercontent.com"));
    }

    #[test]
    fn parses_oauth_fragment() {
        let f = parse_fragment("https://x.chromiumapp.org/#access_token=ya29.a%2Fb&expires_in=3599&token_type=Bearer");
        assert!(f.contains(&("access_token".into(), "ya29.a/b".into())));
        assert!(f.contains(&("expires_in".into(), "3599".into())));
        assert!(parse_fragment("https://x.chromiumapp.org/").is_empty());
    }

    #[test]
    fn formats_ago() {
        assert_eq!(ago(0.2), "剛剛");
        assert_eq!(ago(5.0), "5 分鐘前");
        assert_eq!(ago(125.0), "2 小時前");
        assert_eq!(ago(3000.0), "2 天前");
    }

    #[test]
    fn video_json_is_compatible_with_js_version() {
        let v: Video = serde_json::from_str(r#"{"id":"x","title":"t","author":"a","addedAt":1.0}"#).unwrap();
        assert_eq!(v.added_at, Some(1.0));
        let v: Video = serde_json::from_str(r#"{"id":"x","title":"t"}"#).unwrap();
        assert_eq!(v.author, "");
    }

    #[test]
    fn normalize_urls() {
        assert_eq!(normalize_url(" youtube.com/watch?v=1 ").as_deref(), Some("https://youtube.com/watch?v=1"));
        assert_eq!(normalize_url("http://a.b/c").as_deref(), Some("http://a.b/c"));
        assert_eq!(normalize_url("chrome://settings").as_deref(), Some("chrome://settings"));
        assert_eq!(normalize_url("localhost:3000").as_deref(), Some("https://localhost:3000"));
        assert_eq!(normalize_url("javascript:alert(1)").as_deref(), Some("javascript:alert(1)"));
        assert_eq!(normalize_url(""), None);
        assert_eq!(normalize_url("hello world"), None);
        assert_eq!(normalize_url("hello"), None);
    }

    #[test]
    fn move_items() {
        let ids = |v: &Vec<&str>| v.join("");
        let mut v = vec!["a", "b", "c", "d"];
        assert!(move_by_key(&mut v, |x| x, "a", "c", true));
        assert_eq!(ids(&v), "bcad");
        assert!(move_by_key(&mut v, |x| x, "d", "b", false));
        assert_eq!(ids(&v), "dbca");
        assert!(move_by_key(&mut v, |x| x, "b", "a", true));
        assert_eq!(ids(&v), "dcab");
        assert!(!move_by_key(&mut v, |x| x, "b", "b", true));
        assert!(!move_by_key(&mut v, |x| x, "z", "b", true));
        assert!(!move_by_key(&mut v, |x| x, "b", "z", true));
        assert_eq!(ids(&v), "dcab");
    }

    #[test]
    fn search_matches() {
        assert!(matches("", &["anything"]));
        assert!(matches("  ", &[]));
        assert!(matches("github", &["My Repo", "https://GitHub.com/ric2k1"]));
        assert!(matches("論文 第三章", &["寫論文第三章"]));
        assert!(matches("repo ric2k1", &["My Repo", "https://github.com/ric2k1"]));
        assert!(!matches("論文 email", &["寫論文第三章"]));
        assert!(!matches("x", &[]));
    }
}
