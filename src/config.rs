//! 開發者設定

/// Google Cloud「網頁應用程式」OAuth 用戶端 ID。
/// 設成空字串時，使用者第一次按 Continue with Google 會看到設定精靈。
pub const GOOGLE_CLIENT_ID: &str =
    "627463072769-oifqn6c89lfnt2gihs9vi03pdpeo4fdh.apps.googleusercontent.com";

/// 預設收藏：第一次安裝時自動加入（使用者刪掉後不會再出現）
/// (影片 ID, 標題, 頻道)；標題留空的話，加入時用 YouTube oEmbed 查
pub const DEFAULT_VIDEOS: &[(&str, &str, &str)] = &[
    ("CG1llQrJNbE", "🔥Python 開發環境介紹(Colab)🔥", "AI 幫幫忙"),
    ("KTT6R3-rKKY", "", ""),
];

/// 預設收藏的版本：改了 DEFAULT_VIDEOS 就加 1，已安裝的人更新時會套用下面兩項
pub const DEFAULTS_VERSION: u32 = 2;
/// 從預設收藏移除的影片（已安裝的人更新時一併移除）
pub const RETIRED_DEFAULTS: &[&str] = &["z_m0ApESD1E", "Tm_q8c4KzW0"];
/// 這一版新加的預設影片（已安裝的人更新時加入；之前就有的不重複加，使用者刪掉的也不會再加回來）
pub const NEW_DEFAULTS: &[&str] = &["KTT6R3-rKKY"];

/// 每個來源（稍後觀看／播放清單）最多同步幾部
pub const MAX_PER_SOURCE: usize = 1000;
