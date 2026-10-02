//! 開發者設定

/// Google Cloud「網頁應用程式」OAuth 用戶端 ID。
/// 設成空字串時，使用者第一次按 Continue with Google 會看到設定精靈。
pub const GOOGLE_CLIENT_ID: &str =
    "627463072769-oifqn6c89lfnt2gihs9vi03pdpeo4fdh.apps.googleusercontent.com";

/// 預設收藏：第一次安裝時自動加入（使用者刪掉後不會再出現）
/// (影片 ID, 標題, 頻道)
pub const DEFAULT_VIDEOS: &[(&str, &str, &str)] = &[
    ("CG1llQrJNbE", "🔥Python 開發環境介紹(Colab)🔥", "AI 幫幫忙"),
    (
        "z_m0ApESD1E",
        "Seven Weeks in Silicon Valley | Growing Into My Own Voice | Ray Tsai | NTUTEC 台大創創矽谷探索學程",
        "Ray Tsai",
    ),
    (
        "Tm_q8c4KzW0",
        "Ep 04 | 不是等崩溃了才求救——你的心，也需要一个健身房？ Feat. 大鱼、慈恩",
        "PSY by PSY 心理健身房",
    ),
];

/// 每個來源（喜歡的影片／播放清單）最多同步幾部
pub const MAX_PER_SOURCE: usize = 1000;

/// 「開啟」的電腦小幫手 Mac 安裝檔（由 installLauncher repo 的 GitHub Actions 產生，固定下載最新版）
pub const LAUNCHER_PKG_URL: &str =
    "https://github.com/101academyforyou/installLauncher/releases/latest/download/WatchLaterHub-Launcher.pkg";
/// Linux 的安裝說明（installLauncher repo 的 install-launcher.sh）
pub const LAUNCHER_HELP_URL: &str = "https://github.com/101academyforyou/installLauncher#linux";
