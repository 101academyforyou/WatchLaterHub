//! 收藏清單：手動加入 + 預設影片 + YouTube 同步

use crate::chrome::{self, to_js};
use crate::config;
use crate::videos::{dedupe, move_by_key, parse_id, query, Video};

pub const KEY: &str = "videos"; // 手動加入（含預設影片）
pub const YT_KEY: &str = "ytVideos"; // 從 YouTube 帳號同步

pub async fn get_manual() -> Vec<Video> {
    chrome::get_or(KEY, vec![]).await
}

/// 使用者在清單按 ✕ 拿掉的同步影片（下次同步也不會再出現）
pub const YT_HIDDEN_KEY: &str = "ytHidden";

pub async fn get_synced() -> Vec<Video> {
    let list: Vec<Video> = chrome::get_or(YT_KEY, vec![]).await;
    let hidden: Vec<String> = chrome::get_or(YT_HIDDEN_KEY, vec![]).await;
    list.into_iter().filter(|v| !hidden.contains(&v.id)).collect()
}

/// 拿掉一部同步來的影片
pub async fn hide_synced(id: &str) {
    let mut hidden: Vec<String> = chrome::get_or(YT_HIDDEN_KEY, vec![]).await;
    if !hidden.iter().any(|h| h == id) {
        hidden.push(id.to_string());
        // 只留最近的 2000 筆
        let extra = hidden.len().saturating_sub(2000);
        hidden.drain(..extra);
        chrome::set(&[(YT_HIDDEN_KEY, to_js(&hidden))]).await;
    }
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
    add_many_known(texts, &[]).await
}

/// 同 `add_many`，但 `known` 裡已知標題的影片不必再查 oEmbed（播放清單匯入時很多部，會快很多）
pub async fn add_many_known(texts: &[String], known: &[Video]) -> (usize, usize) {
    let mut list = get_manual().await;
    let mut ids: Vec<String> = vec![];
    for id in texts.iter().filter_map(|t| parse_id(t)) {
        if !ids.contains(&id) && !list.iter().any(|v| v.id == id) {
            ids.push(id);
        }
    }
    for id in &ids {
        let (title, author) = match known.iter().find(|v| &v.id == id) {
            Some(v) => (v.title.clone(), v.author.clone()),
            None => fetch_info(id).await,
        };
        list.push(Video { id: id.clone(), title, author, added_at: Some(chrome::now()) });
    }
    save_manual(&list).await;
    (ids.len(), texts.len() - ids.len())
}

/// 拖曳排序手動加入的影片
pub async fn reorder(from: &str, to: &str, after: bool) {
    let mut list = get_manual().await;
    if move_by_key(&mut list, |v| v.id.as_str(), from, to, after) {
        save_manual(&list).await;
    }
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

