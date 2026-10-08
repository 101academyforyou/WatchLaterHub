//! 開發者設定

/// Google Cloud「網頁應用程式」OAuth 用戶端 ID。
/// 設成空字串時，使用者第一次按 Continue with Google 會看到設定精靈。
pub const GOOGLE_CLIENT_ID: &str =
    "627463072769-oifqn6c89lfnt2gihs9vi03pdpeo4fdh.apps.googleusercontent.com";

/// 預設收藏：第一次安裝時自動加入（使用者刪掉後不會再出現）
/// (影片 ID, 標題, 頻道)；標題留空時，加入時用 oEmbed 查
pub const DEFAULT_VIDEOS: &[(&str, &str, &str)] = &[
    ("CG1llQrJNbE", "🔥Python 開發環境介紹(Colab)🔥", "AI 幫幫忙"),
    ("KTT6R3-rKKY", "", ""),
];

/// 預設收藏的版本：改了 DEFAULT_VIDEOS 就加 1，已安裝的使用者更新時會套用下面的增減
pub const DEFAULTS_VERSION: u32 = 2;
/// 舊版的預設影片、已經拿掉的：更新時從收藏移除
pub const REMOVED_DEFAULTS: &[&str] = &["z_m0ApESD1E", "Tm_q8c4KzW0"];
/// 第 1 版就有的預設影片：更新時不重新加入（使用者可能自己刪掉了）
pub const V1_DEFAULTS: &[&str] = &["CG1llQrJNbE", "z_m0ApESD1E", "Tm_q8c4KzW0"];

/// 每個來源（稍後觀看／播放清單）最多同步幾部
pub const MAX_PER_SOURCE: usize = 1000;
