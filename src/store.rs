//! 收藏清單：手動加入 + 預設影片 + YouTube 同步

use crate::chrome::{self, to_js};
use crate::config;
use crate::videos::{dedupe, parse_id, query, Video};

pub const KEY: &str = "videos"; // 手動加入（含預設影片）
pub const YT_KEY: &str = "ytVideos"; // 從 YouTube 帳號同步

pub async fn get_manual() -> Vec<Video> {
    chrome::get_or(KEY, vec![]).await
}

pub async fn get_synced() -> Vec<Video> {
    chrome::get_or(YT_KEY, vec![]).await
}

/// 手動 + YouTube 同步，去除重複
pub async fn get_all() -> Vec<Video> {
    let mut all = get_manual().await;
    all.extend(get_synced().await);
    dedupe(all)
}

async fn save_manual(list: &[Video]) {
    chrome::set(&[(KEY, to_js(list))]).await;
}

/// 用 oEmbed 取得標題與頻道名（不需要登入）
async fn fetch_info(id: &str) -> (String, String) {
    let url = format!(
        "https://www.youtube.com/oembed?{}",
        query(&[("format", "json"), ("url", &format!("https://www.youtube.com/watch?v={id}"))])
    );
    match chrome::fetch(&url, None, "GET").await {
        Ok(r) if r.status == 200 => (
            r.json["title"].as_str().unwrap_or(id).to_string(),
            r.json["author_name"].as_str().unwrap_or("").to_string(),
        ),
        _ => (format!("YouTube 影片 {id}"), String::new()),
    }
}

/// 加入多個網址，回傳 (加入數, 略過數)
pub async fn add_many(texts: &[String]) -> (usize, usize) {
    let mut list = get_manual().await;
    let mut ids: Vec<String> = vec![];
    for id in texts.iter().filter_map(|t| parse_id(t)) {
        if !ids.contains(&id) && !list.iter().any(|v| v.id == id) {
            ids.push(id);
        }
    }
    for id in &ids {
        let (title, author) = fetch_info(id).await;
        list.push(Video { id: id.clone(), title, author, added_at: Some(chrome::now()) });
    }
    save_manual(&list).await;
    (ids.len(), texts.len() - ids.len())
}

pub async fn remove(id: &str) {
    let list: Vec<Video> = get_manual().await.into_iter().filter(|v| v.id != id).collect();
    save_manual(&list).await;
}

/// 第一次安裝時放入預設收藏（只做一次）
pub async fn seed_defaults() {
    if chrome::get_or("defaultsSeeded", false).await {
        return;
    }
    let mut list = get_manual().await;
    for (id, title, author) in config::DEFAULT_VIDEOS {
        if !list.iter().any(|v| v.id == *id) {
            list.push(Video { added_at: Some(chrome::now()), ..Video::new(id, title, author) });
        }
    }
    chrome::set(&[(KEY, to_js(&list)), ("defaultsSeeded", to_js(&true))]).await;
}

