//! 英文版：畫面上的中文在顯示前換成英文
//!
//! 程式裡的文字都維持中文；英文模式時，用 MutationObserver 監看整個頁面，
//! 任何新出現或改變的文字、title／placeholder／aria-label 都查下面的對照表換掉。
//! - `EXACT`：整句對照
//! - `PATTERNS`：含變數的句子，`{名稱}` 是變數，`{名稱:過濾}` 會先檢查／轉換
//!   （wd 星期、mon 月份、ampm 上午下午、n 數字）；其餘變數會再翻譯一次
//! - 使用者自己的內容（書籤名稱、待辦文字…）放在有 `data-nt` 的元素裡，不翻譯

use std::cell::Cell;

/// 語言設定存在 localStorage（同步讀取，畫面不會先閃中文）和 chrome.storage（背景頁用）
pub const LANG_KEY: &str = "lang";

thread_local! {
    static EN: Cell<bool> = const { Cell::new(false) };
}

pub fn set_en(en: bool) {
    EN.with(|e| e.set(en));
}

pub fn is_en() -> bool {
    EN.with(|e| e.get())
}

fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '\u{3000}'..='\u{30ff}' | '\u{4e00}'..='\u{9fff}' | '\u{ff00}'..='\u{ffef}'))
}

/// 整句對照（左：中文，右：英文）
const EXACT: &[(&str, &str)] = &[
    // 頂列
    ("☰ 收藏", "☰ Saved"),
    ("收藏", "Saved"),
    ("待辦清單", "To-do list"),
    ("開啟電腦上的軟體", "Open apps on your computer"),
    ("⏺︎ 工具", "⏺︎ Tools"),
    ("工具", "Tools"),
    ("顯示／隱藏書籤", "Show/hide bookmarks"),
    ("★ 書籤", "★ Bookmarks"),
    ("最近瀏覽", "Recently visited"),
    ("最近", "Recent"),
    ("YouTube 帳號", "YouTube account"),
    ("書籤列", "Bookmarks bar"),
    ("其他書籤列項目", "More bookmarks"),
    ("其他書籤", "Other bookmarks"),
    ("行動裝置書籤", "Mobile bookmarks"),
    ("切換語言", "Switch language"),
    // 搜尋與影片
    ("搜尋", "Search"),
    ("以 Google AI 模式搜尋", "Search with Google AI Mode"),
    ("✦ AI 模式", "✦ AI Mode"),
    ("Google 搜尋", "Google Search"),
    ("好手氣", "I'm Feeling Lucky"),
    ("換一部", "Another video"),
    ("在新分頁播放", "Play in a new tab"),
    ("⟳ 換一部", "⟳ Another"),
    ("正在載入你的 YouTube 收藏…", "Loading your YouTube favorites…"),
    ("第一次同步可能需要幾秒鐘", "The first sync may take a few seconds"),
    ("你的清單裡還沒有影片", "There are no videos in your lists yet"),
    (
        "把影片加到 YouTube 的「稍後觀看」或播放清單，再到「收藏」勾選。",
        "Add videos to Watch later or a playlist on YouTube, then tick it under “Saved”.",
    ),
    ("每開一個分頁，重溫一部你收藏的影片", "Every new tab, rediscover a video you saved"),
    (
        "用 Google 帳號登入，自動載入你在 YouTube 的稍後觀看與播放清單。",
        "Sign in with Google to load your Watch later list and playlists from YouTube.",
    ),
    ("Google 拒絕了登入要求，通常是 OAuth 用戶端設定不對：", "Google rejected the sign-in. Usually the OAuth client is misconfigured:"),
    ("用戶端類型要是「網頁應用程式」", "The client type must be “Web application”"),
    ("config.rs 的 Client ID 要屬於這個用戶端", "The Client ID in config.rs must belong to this client"),
    ("剛建立或修改的用戶端要等 5 分鐘以上才會生效", "A newly created or edited client takes 5+ minutes to take effect"),
    // 收藏視窗
    ("我的收藏", "My saved videos"),
    ("在新分頁打開 YouTube", "Open YouTube in a new tab"),
    ("登入後，可以選擇哪些播放清單要自動同步載入。", "After signing in, choose which playlists to sync automatically."),
    ("勾選要自動同步載入的播放清單（勾選後立即同步）", "Tick the playlists to sync (they sync right away)"),
    ("勾選要同步的播放清單，會立即同步載入", "Tick the playlists to sync; they load right away"),
    ("登出", "Sign out"),
    ("手動加入", "Add manually"),
    ("貼上播放清單網址，一次取出所有影片連結", "Paste a playlist URL to get all its video links"),
    ("取出連結", "Get links"),
    ("貼上 YouTube 網址，一行一個（可一次貼很多）", "Paste YouTube URLs, one per line (many at once is fine)"),
    ("複製", "Copy"),
    ("加入", "Add"),
    ("搜尋影片（標題、頻道）", "Search videos (title, channel)"),
    ("搜尋影片", "Search videos"),
    ("YouTube 同步", "YouTube sync"),
    ("從清單移除（之後同步也不會再加入）", "Remove from list (won't be re-added by sync)"),
    ("尚未同步", "Not synced yet"),
    ("同步中…", "Syncing…"),
    ("請貼上播放清單網址（網址裡要有 list=…）", "Paste a playlist URL (it must contain list=…)"),
    ("讀取播放清單中…", "Reading playlist…"),
    ("已複製 ✓", "Copied ✓"),
    ("加入中…", "Adding…"),
    ("沒有可複製的連結", "No links to copy"),
    ("登出後會清除已同步的影片（手動加入的會保留）。確定嗎？", "Signing out clears synced videos (manually added ones stay). Continue?"),
    ("稍後觀看", "Watch later"),
    ("喜歡的影片", "Liked videos"),
    ("還沒有影片", "No videos yet"),
    ("找不到符合的影片", "No matching videos"),
    ("移除", "Remove"),
    // Google 登入設定
    ("啟用 Google 登入（只需設定一次）", "Enable Google sign-in (one-time setup)"),
    (
        "Google 規定每個「用 Google 登入」的應用程式都要有一組 OAuth Client ID。照下面 4 步做完，之後按一下就能選帳號登入。",
        "Google requires every “Sign in with Google” app to have an OAuth Client ID. Finish the 4 steps below, then sign in with one click.",
    ),
    ("啟用 YouTube API", "Enable the YouTube API"),
    ("開啟 → 按「啟用」", "Open → click “Enable”"),
    ("第一次使用會先要你建立專案，名稱隨意。", "The first time, you'll be asked to create a project; any name works."),
    ("設定 OAuth 同意畫面", "Set up the OAuth consent screen"),
    ("開啟 → 按「開始」", "Open → click “Get started”"),
    (
        "選「外部」，填應用程式名稱與 email。在「目標對象」把你自己的 Google 帳號加入測試使用者。",
        "Choose “External” and fill in the app name and email. Under “Audience”, add your own Google account as a test user.",
    ),
    ("建立 OAuth 用戶端", "Create an OAuth client"),
    ("開啟", "Open"),
    ("應用程式類型選「", "For application type, choose “"),
    ("網頁應用程式", "Web application"),
    ("」，在「已授權的重新導向 URI」貼上：", "”, and under “Authorized redirect URIs” paste:"),
    ("貼上用戶端 ID", "Paste the client ID"),
    ("儲存並 Continue with Google", "Save and Continue with Google"),
    ("授權已失效，請重新登入", "Authorization expired, please sign in again"),
    ("已取消登入", "Sign-in cancelled"),
    ("格式不對，應該長得像 1234-abc.apps.googleusercontent.com", "Wrong format; it should look like 1234-abc.apps.googleusercontent.com"),
    ("尚未設定 Client ID", "Client ID not set"),
    ("登入沒有取得 access token", "Sign-in returned no access token"),
    ("登入沒有回傳結果", "Sign-in returned nothing"),
    ("未知錯誤", "Unknown error"),
    ("fetch 回傳格式錯誤", "Unexpected fetch response"),
    ("建立資料夾沒有回傳 id", "Creating the folder returned no id"),
    // 播放清單
    ("讀不到「稍後觀看」：請先在這個 Chrome 登入 YouTube（youtube.com）", "Can't read “Watch later”: sign in to YouTube (youtube.com) in this Chrome first"),
    ("讀不到播放清單內容（YouTube 可能改版了）", "Couldn't read the playlist (YouTube may have changed)"),
    (
        "這個播放清單不存在，或是「私人」清單（只有「公開」或「不公開」的清單能讀取）",
        "This playlist doesn't exist or is private (only public or unlisted playlists can be read)",
    ),
    ("這個清單裡沒有可播放的影片（可能是私人清單，或影片都被刪除了）", "This list has no playable videos (it may be private, or the videos were deleted)"),
    // TODO
    ("TODO 清單", "TODO list"),
    ("打開任務矩陣（緊急／重要）", "Open the task matrix (urgent / important)"),
    ("✦ 任務矩陣", "✦ Task matrix"),
    ("任務矩陣", "Task matrix"),
    ("新增待辦事項，按 Enter 加入", "New to-do, press Enter to add"),
    ("搜尋待辦事項", "Search to-dos"),
    ("把 TODO 清單下載成 CSV（可用 Excel 開啟）", "Download the TODO list as CSV (opens in Excel)"),
    ("匯出 CSV", "Export CSV"),
    ("清除已完成", "Clear completed"),
    ("還沒有待辦事項，在上面輸入後按 Enter", "No to-dos yet. Type above and press Enter"),
    ("找不到符合的待辦事項", "No matching to-dos"),
    ("點兩下編輯", "Double-click to edit"),
    ("優先順序", "Priority"),
    ("筆記與圖片", "Notes & images"),
    ("打開筆記（有內容）", "Open notes (has content)"),
    ("設定提醒時間", "Set a reminder"),
    ("刪除", "Delete"),
    ("高優先", "High priority"),
    ("中優先", "Medium priority"),
    ("低優先", "Low priority"),
    ("再點一下取消", "Click again to clear"),
    ("任務矩陣分類（點標題列的「任務矩陣」調整）", "Task matrix category (change it via “Task matrix” in the header)"),
    ("修改提醒時間", "Change reminder time"),
    ("移回未分類", "Move back to uncategorized"),
    ("刪除這項待辦", "Delete this to-do"),
    ("所有待辦都分類好了 👍", "All to-dos are sorted 👍"),
    ("拖曳待辦到這裡，或在下方新增", "Drag to-dos here, or add one below"),
    ("把待辦拖進象限，或直接在象限裡新增", "Drag to-dos into a quadrant, or add them right in a quadrant"),
    ("未分類的待辦", "Uncategorized to-dos"),
    ("緊急", "Urgent"),
    ("不緊急", "Not urgent"),
    ("重要", "Important"),
    ("不重要", "Not important"),
    ("重要且緊急", "Important & urgent"),
    ("立即去做", "Do it now"),
    ("重要不緊急", "Important, not urgent"),
    ("排時間做", "Schedule it"),
    ("緊急不重要", "Urgent, not important"),
    ("盡快處理或交給別人", "Handle quickly or delegate"),
    ("不緊急不重要", "Neither urgent nor important"),
    ("少做或刪除", "Do less or drop it"),
    ("＋ 新增，按 Enter", "＋ Add, press Enter"),
    ("已完成", "Done"),
    ("未完成", "Not done"),
    ("待辦事項", "To-do"),
    ("狀態", "Status"),
    ("提醒時間", "Reminder"),
    ("5 分鐘後", "In 5 min"),
    ("10 分鐘後", "In 10 min"),
    ("30 分鐘後", "In 30 min"),
    ("從月曆選擇", "Pick from calendar"),
    ("提醒時間（可直接輸入，例如 2026/10/02 20:30）", "Reminder time (type it, e.g. 2026/10/02 20:30)"),
    ("設定", "Set"),
    ("取消提醒", "Remove reminder"),
    ("請輸入未來的時間", "Please enter a time in the future"),
    ("看不懂這個時間，請照「2026/10/02 20:30」的格式輸入", "Couldn't read that time; use the format “2026/10/02 20:30”"),
    ("今天", "Today"),
    ("明天", "Tomorrow"),
    ("昨天", "Yesterday"),
    ("前天", "2 days ago"),
    ("本週", "This week"),
    ("日", "Sun"),
    ("一", "Mon"),
    ("二", "Tue"),
    ("三", "Wed"),
    ("四", "Thu"),
    ("五", "Fri"),
    ("六", "Sat"),
    // TODO 筆記
    ("📝 筆記", "📝 Notes"),
    ("筆記", "Notes"),
    ("主題", "Title"),
    ("待辦事項主題", "To-do title"),
    ("寫更多筆記…（可以直接貼上圖片，或把圖片、PDF 拖進來）", "Write more notes… (paste images, or drag in images and PDFs)"),
    ("📎 上傳圖片／PDF", "📎 Upload images / PDF"),
    ("完成", "Done"),
    ("點一下關閉", "Click to close"),
    ("點一下放大", "Click to enlarge"),
    ("移除圖片", "Remove image"),
    ("加入圖片中…", "Adding images…"),
    ("加入檔案中…", "Adding files…"),
    ("找不到這個檔案（可能已被刪除）", "File not found (it may have been deleted)"),
    ("圖片大小", "Image size"),
    ("原始", "Original"),
    ("原始大小", "Original size"),
    ("放大檢視", "View larger"),
    ("拖曳調整大小", "Drag to resize"),
    // 計時器
    ("碼表", "Stopwatch"),
    ("倒數計時", "Countdown"),
    ("計時器", "Timer"),
    ("點一下打開計時器", "Click to open the timer"),
    ("關閉", "Close"),
    ("⏱ 倒數計時", "⏱ Countdown"),
    ("⏲ 碼表", "⏲ Stopwatch"),
    ("自訂：分鐘（如 15、2.5）或 分:秒（如 1:30）", "Custom: minutes (e.g. 15, 2.5) or min:sec (e.g. 1:30)"),
    ("自訂倒數時間", "Custom countdown"),
    ("開始", "Start"),
    ("取消", "Cancel"),
    ("計次", "Lap"),
    ("▶ 開始", "▶ Start"),
    ("把計次紀錄下載成 CSV（可用 Excel 開啟）", "Download laps as CSV (opens in Excel)"),
    ("⏸ 暫停", "⏸ Pause"),
    ("▶ 繼續", "▶ Resume"),
    ("重設", "Reset"),
    ("＋ 筆記", "＋ Note"),
    ("移除這筆計次", "Remove this lap"),
    ("時間到！", "Time's up!"),
    ("（已暫停）", " (paused)"),
    ("請輸入 1 秒到 24 小時，例如：15、2.5、1:30、90秒", "Enter 1 second to 24 hours, e.g. 15, 2.5, 1:30, 90s"),
    ("拖曳調整順序", "Drag to reorder"),
    ("分段時間", "Split"),
    ("累計時間", "Total"),
    ("總時間", "Total time"),
    // 天氣
    ("選擇地區", "Choose location"),
    ("輸入城市，例如：台中、高雄、東京", "Enter a city, e.g. Taichung, Kaohsiung, Tokyo"),
    ("使用目前位置", "Use current location"),
    ("目前位置", "Current location"),
    ("天氣", "Weather"),
    ("氣溫", "Temperature"),
    ("降雨機率", "Precipitation"),
    ("風向/風速", "Wind"),
    ("逐時預報", "Hourly forecast"),
    ("點一下查看詳細天氣", "Click for detailed weather"),
    ("搜尋中…", "Searching…"),
    ("找不到這個地方，換個名稱試試（例如：臺中、Taichung、東京）", "Couldn't find that place; try another name (e.g. Taichung, Tokyo)"),
    ("臺北市", "Taipei"),
    ("晴朗", "Clear"),
    ("大致晴朗", "Mostly clear"),
    ("多雲", "Partly cloudy"),
    ("陰天", "Overcast"),
    ("霧", "Fog"),
    ("毛毛雨", "Drizzle"),
    ("雨", "Rain"),
    ("大雨", "Heavy rain"),
    ("凍雨", "Freezing rain"),
    ("雪", "Snow"),
    ("雷雨", "Thunderstorm"),
    ("雷雨冰雹", "Thunderstorm with hail"),
    // 工具
    ("常用軟體", "Favorite apps"),
    ("＋ 新增", "＋ Add"),
    ("搜尋電腦上的軟體", "Search apps on your computer"),
    ("名稱", "Name"),
    ("網址，例如 vscode:// 或 https://…", "URL, e.g. vscode:// or https://…"),
    ("小幫手沒有回應", "The helper app didn't respond"),
    ("取消常用", "Unpin"),
    ("把軟體拖到這裡設為常用", "Drag an app here to pin it"),
    ("電腦上的軟體", "Apps on your computer"),
    ("還沒有加入軟體，按右上「＋ 新增」", "No apps yet. Click “＋ Add” at the top right"),
    ("找不到符合的軟體", "No matching apps"),
    ("網址要包含通訊協定，例如 vscode:// 或 https://", "The URL needs a scheme, e.g. vscode:// or https://"),
    // 最近
    ("期間", "Period"),
    ("自訂", "Custom"),
    ("清空這個清單（不會刪除瀏覽紀錄）", "Clear this list (browsing history is kept)"),
    ("清空", "Clear"),
    ("搜尋最近瀏覽", "Search recent pages"),
    ("從", "From"),
    ("開始日期", "Start date"),
    ("開始（時）", "Start (hour)"),
    ("開始（分）", "Start (minute)"),
    ("到", "To"),
    ("結束日期", "End date"),
    ("結束（時）", "End (hour)"),
    ("結束（分）", "End (minute)"),
    ("套用", "Apply"),
    ("這段期間沒有瀏覽紀錄", "No history in this period"),
    ("還沒有瀏覽紀錄", "No browsing history yet"),
    ("找不到符合的網頁", "No matching pages"),
    ("已加入書籤（點一下編輯）", "Bookmarked (click to edit)"),
    ("從清單移除（不會刪除瀏覽紀錄）", "Remove from list (history is kept)"),
    // 書籤
    ("書籤", "Bookmarks"),
    ("釘選的書籤", "Pinned bookmarks"),
    ("☆ 加入書籤", "☆ Bookmark"),
    ("★ 已加入書籤", "★ Bookmarked"),
    ("隱藏書籤", "Hide bookmarks"),
    ("搜尋書籤", "Search bookmarks"),
    ("復原", "Undo"),
    ("加入書籤", "Add bookmark"),
    ("編輯書籤", "Edit bookmark"),
    ("書籤名稱", "Bookmark name"),
    ("網址", "URL"),
    ("資料夾", "Folder"),
    ("＋ 新資料夾", "＋ New folder"),
    ("新資料夾名稱", "New folder name"),
    ("資料夾名稱", "Folder name"),
    ("建立", "Create"),
    ("加入到資料夾最上方", "Add to the top of the folder"),
    ("移到資料夾最上方", "Move to the top of the folder"),
    ("儲存", "Save"),
    ("編輯這部影片的書籤", "Edit this video's bookmark"),
    ("新增書籤（預先填入目前這部影片）", "Add a bookmark (prefilled with this video)"),
    ("（未命名資料夾）", "(Untitled folder)"),
    ("請輸入有效的網址，例如 https://www.google.com", "Enter a valid URL, e.g. https://www.google.com"),
    ("編輯書籤（名稱、網址、資料夾）", "Edit bookmark (name, URL, folder)"),
    ("移除書籤", "Remove bookmark"),
    ("書籤小程式無法在新分頁執行", "Bookmarklets can't run on the new tab page"),
    ("找不到符合的書籤", "No matching bookmarks"),
    ("還沒有書籤。", "No bookmarks yet."),
    ("按上方「☆ 加入書籤」新增。", "Click “☆ Bookmark” above to add one."),
    ("取消釘選", "Unpin"),
    ("把書籤拖到這裡釘選", "Drag a bookmark here to pin it"),
    ("資料夾不能釘選，請拖一個書籤", "Folders can't be pinned; drag a bookmark"),
    ("‹ 上一層", "‹ Back"),
    ("（空的）", "(Empty)"),
    ("（右鍵編輯，可拖曳）", "(right-click to edit, drag to move)"),
    ("編輯名稱", "Rename"),
    // 提醒小視窗
    ("⏰ TODO 提醒", "⏰ TODO reminder"),
    ("TODO 提醒", "TODO reminder"),
    ("再計時：", "Again:"),
    ("✓ 完成", "✓ Done"),
    ("稍後再提醒：", "Remind me in:"),
    ("1 分鐘", "1 min"),
    ("5 分鐘", "5 min"),
    ("10 分鐘", "10 min"),
    ("25 分鐘", "25 min"),
    ("30 分鐘", "30 min"),
    ("1 小時", "1 hour"),
    ("這個待辦事項已經完成或被刪除了。", "This to-do is already done or was deleted."),
    ("打開 TODO 清單", "Open TODO list"),
    ("知道了", "Got it"),
    // 右鍵選單
    ("加入 WatchLaterHub", "Add to WatchLaterHub"),
    ("把這部影片加入 WatchLaterHub", "Add this video to WatchLaterHub"),
    // 其他
    ("剛剛", "just now"),
    ("沒有影片", "No videos"),
];

/// 含變數的句子（左：中文樣式，右：英文樣式）
const PATTERNS: &[(&str, &str)] = &[
    // 收藏
    ("已同步 {n:n} 部 · {t}", "Synced {n} · {t}"),
    ("已登入：{x}", "Signed in: {x}"),
    ("{x}（YouTube 帳號）", "{x} (YouTube account)"),
    ("{a:n} / {b:n} 部", "{a} / {b} videos"),
    ("{n:n} 部", "{n} videos"),
    ("{n:n} 部影片", "{n} videos"),
    ("讀取播放清單中…已找到 {n:n} 部", "Reading playlist… found {n}"),
    ("已取出 {n:n} 部影片的連結，可按「複製」或直接「加入」", "Got links to {n} videos. Click “Copy”, or “Add” them directly"),
    ("已加入 {a:n} 部，略過 {b:n} 筆（重複或無效）", "Added {a}, skipped {b} (duplicates or invalid)"),
    ("已加入 {a:n} 部", "Added {a}"),
    ("已複製 {n:n} 個連結 ✓", "Copied {n} links ✓"),
    ("YouTube 影片 {x}", "YouTube video {x}"),
    ("已授權的重新導向 URI 要完全等於 {x}", "The authorized redirect URI must be exactly {x}"),
    ("打不開播放清單（HTTP {n}）", "Couldn't open the playlist (HTTP {n})"),
    ("登入失敗：{e}", "Sign-in failed: {e}"),
    ("YouTube API 錯誤 {n}", "YouTube API error {n}"),
    // 時間
    ("{m:n} 分鐘前", "{m} min ago"),
    ("{h:n} 小時前", "{h} hr ago"),
    ("{d:n} 天前", "{d} days ago"),
    ("今天 {t}", "Today {t}"),
    ("明天 {t}", "Tomorrow {t}"),
    ("昨天 {t}", "Yesterday {t}"),
    ("{d} (週{w:wd}) {t}", "{w} {d} {t}"),
    ("{d} (週{w:wd})", "{w} {d}"),
    ("{m:mon}月{d:n}日 週{w:wd} {t}", "{w}, {m} {d} {t}"),
    ("週{w:wd}", "{w}"),
    ("星期{w:wd}{a:ampm}{h:n}:00", "{w} {h}:00 {a}"),
    ("星期{w:wd}", "{w}"),
    ("{a:ampm}{h:n}時", "{h} {a}"),
    ("{s:n} 秒", "{s} sec"),
    ("{m:n} 分鐘", "{m} min"),
    ("{m:n} 分 {s:n} 秒", "{m} min {s} sec"),
    ("{h:n} 小時", "{h} hr"),
    ("{h:n} 小時 {m:n} 分鐘", "{h} hr {m} min"),
    ("{h:n} 小時 {m:n} 分 {s:n} 秒", "{h} hr {m} min {s} sec"),
    // 計時器
    ("{d}計時（已暫停）", "{d} timer (paused)"),
    ("{d}計時", "{d} timer"),
    ("{d}計時結束！", "{d} timer finished!"),
    ("{d}計時結束", "{d} timer finished"),
    ("提醒時間：{t}", "Reminder: {t}"),
    ("移除計次 {n:n}", "Remove lap {n}"),
    ("計次 {n:n} 的筆記", "Note for lap {n}"),
    ("計次 {n:n}", "Lap {n}"),
    ("碼表計次-{x}", "stopwatch-laps-{x}"),
    // TODO
    ("剩 {n:n} 項 · 已完成 {d:n} 項", "{n} left · {d} done"),
    ("設為{x}", "Set {x}"),
    ("完成：{x}", "Done: {x}"),
    ("{t} · 已到期", "{t} · overdue"),
    ("（{n:n}）", " ({n})"),
    // 書籤與最近
    ("會建立在「{x}」裡", "Will be created in “{x}”"),
    ("無法建立資料夾：{e}", "Couldn't create the folder: {e}"),
    ("編輯書籤：{x}", "Edit bookmark: {x}"),
    ("移除書籤：{x}", "Remove bookmark: {x}"),
    ("無法移動：{e}", "Couldn't move: {e}"),
    ("無法放進書籤：{e}", "Couldn't add to bookmarks: {e}"),
    ("無法移除：{e}", "Couldn't remove: {e}"),
    ("已移除「{x}」", "Removed “{x}”"),
    ("無法復原：{e}", "Couldn't undo: {e}"),
    ("讀不到書籤：{e}", "Couldn't read bookmarks: {e}"),
    ("取消釘選：{x}", "Unpin: {x}"),
    ("從最近移除：{x}", "Remove from recent: {x}"),
    ("編輯名稱：{x}", "Rename: {x}"),
    ("檔案太大：{x}（上限 30 MB）", "File too large: {x} (max 30 MB)"),
    ("{a}：{b}", "{a}: {b}"),
    // 工具
    ("已開啟「{x}」", "Opened “{x}”"),
    ("無法開啟「{x}」：{e}", "Couldn't open “{x}”: {e}"),
    ("無法開啟「{x}」", "Couldn't open “{x}”"),
    ("開啟：{x}", "Open: {x}"),
    ("取消常用：{x}", "Unpin: {x}"),
    // 天氣
    ("降雨機率：{n}%", "Precipitation: {n}%"),
    ("濕度：{n}%", "Humidity: {n}%"),
    ("風速：{n} 公里/時", "Wind: {n} km/h"),
    ("{n} 公里/時", "{n} km/h"),
    ("{x}（取不到你的位置）", "{x} (couldn't get your location)"),
];

const WEEKDAYS: [(&str, &str); 7] =
    [("日", "Sun"), ("一", "Mon"), ("二", "Tue"), ("三", "Wed"), ("四", "Thu"), ("五", "Fri"), ("六", "Sat")];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// 變數的過濾：檢查並轉換（不符合時這個樣式就不算數）
fn apply_filter(filter: &str, v: &str) -> Option<String> {
    match filter {
        "wd" => WEEKDAYS.iter().find(|(z, _)| *z == v).map(|(_, e)| e.to_string()),
        "mon" => v.parse::<usize>().ok().filter(|m| (1..=12).contains(m)).map(|m| MONTHS[m - 1].to_string()),
        "ampm" => match v {
            "上午" => Some("AM".into()),
            "下午" => Some("PM".into()),
            _ => None,
        },
        "n" => (!v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | ':' | '-' | '+'))).then(|| v.to_string()),
        _ => Some(translate(v)),
    }
}

enum Seg<'a> {
    Lit(&'a str),
    Var(&'a str, &'a str),
}

fn parse(p: &str) -> Vec<Seg<'_>> {
    let mut out = vec![];
    let mut rest = p;
    while let Some(i) = rest.find('{') {
        if i > 0 {
            out.push(Seg::Lit(&rest[..i]));
        }
        let end = rest[i..].find('}').map(|j| i + j).unwrap_or(rest.len() - 1);
        let inner = &rest[i + 1..end];
        let (name, filter) = inner.split_once(':').unwrap_or((inner, ""));
        out.push(Seg::Var(name, filter));
        rest = &rest[end + 1..];
    }
    if !rest.is_empty() {
        out.push(Seg::Lit(rest));
    }
    out
}

/// 用樣式比對整句；成功時回傳 (變數名稱, 轉換後的值)
fn match_pattern(segs: &[Seg], s: &str) -> Option<Vec<(String, String)>> {
    match segs.split_first() {
        None => s.is_empty().then(Vec::new),
        Some((Seg::Lit(l), rest)) => match_pattern(rest, s.strip_prefix(l)?),
        Some((Seg::Var(name, filter), rest)) => {
            // 變數至少一個字；由短到長試
            for (i, _) in s.char_indices().skip(1).chain(std::iter::once((s.len(), ' '))) {
                if s.is_empty() {
                    break;
                }
                let (v, tail) = s.split_at(i);
                let Some(mut caps) = match_pattern(rest, tail) else { continue };
                let Some(val) = apply_filter(filter, v) else { continue };
                caps.insert(0, (name.to_string(), val));
                return Some(caps);
            }
            None
        }
    }
}

fn fill(template: &str, caps: &[(String, String)]) -> String {
    let mut out = template.to_string();
    for (k, v) in caps {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

fn exact(s: &str) -> Option<&'static str> {
    EXACT.iter().find(|(z, _)| *z == s).map(|(_, e)| *e)
}

/// 樣式依「固定文字」長短排序，先比對比較具體的
fn sorted_patterns() -> &'static Vec<(&'static str, &'static str)> {
    thread_local! {
        static SORTED: &'static Vec<(&'static str, &'static str)> = {
            let mut v: Vec<(&str, &str)> = PATTERNS.to_vec();
            let lit_len = |p: &str| parse(p).iter().map(|s| if let Seg::Lit(l) = s { l.chars().count() } else { 0 }).sum::<usize>();
            v.sort_by_key(|(z, _)| std::cmp::Reverse(lit_len(z)));
            Box::leak(Box::new(v))
        };
    }
    SORTED.with(|s| *s)
}

/// 回傳 (譯文, 是否完全沒有中文了)。有變數的樣式若譯完還有中文，先記著，再試別的樣式
fn translate_core(s: &str) -> Option<(String, bool)> {
    if let Some(e) = exact(s) {
        return Some((e.to_string(), true));
    }
    let mut partial = None;
    for (z, e) in sorted_patterns() {
        if let Some(caps) = match_pattern(&parse(z), s) {
            let out = fill(e, &caps);
            if !has_cjk(&out) {
                return Some((out, true));
            }
            partial.get_or_insert(out);
        }
    }
    partial.map(|p| (p, false))
}

/// 把一段中文翻成英文；翻不出來就原樣回傳。不管目前是不是英文模式（測試用）
pub fn translate(s: &str) -> String {
    if !has_cjk(s) {
        return s.to_string();
    }
    // 保留前後空白
    let start = s.len() - s.trim_start().len();
    let end = s.trim_end().len();
    let (lead, body, trail) = (&s[..start], &s[start..end], &s[end..]);
    // 多行：一行一行翻
    if body.contains('\n') {
        let lines: Vec<String> = body.split('\n').map(translate).collect();
        return format!("{lead}{}{trail}", lines.join("\n"));
    }
    let whole = translate_core(body);
    if let Some((t, true)) = &whole {
        return format!("{lead}{t}{trail}");
    }
    // 去掉開頭的符號（⚠ ★ • 🕒 …）再試
    let is_sym = |c: char| !(c.is_alphanumeric() || has_cjk(&c.to_string()));
    let core_start = body.char_indices().find(|(_, c)| !is_sym(*c)).map(|(i, _)| i).unwrap_or(0);
    if core_start > 0 {
        let (sym, core) = body.split_at(core_start);
        if let Some((t, _)) = translate_core(core) {
            return format!("{lead}{sym}{t}{trail}");
        }
    }
    match whole {
        Some((t, _)) => format!("{lead}{t}{trail}"),
        None => s.to_string(),
    }
}

/// 只查整句對照表（CSV 用，避免把使用者的內容當成句型翻譯）
pub fn exact_only(s: &str) -> String {
    exact(s).map(String::from).unwrap_or_else(|| s.to_string())
}

/// 英文模式時翻譯，中文模式原樣
pub fn tr(s: &str) -> String {
    if is_en() {
        translate(s)
    } else {
        s.to_string()
    }
}

// ---------- 頁面 ----------

pub mod page {
    use super::*;
    use crate::ui::{doc, listen};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{Element, MutationObserver, MutationObserverInit, MutationRecord, Node};

    const ATTRS: [&str; 5] = ["title", "placeholder", "aria-label", "alt", "data-placeholder"];

    fn local_storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    /// 設定的語言；沒設定過就看瀏覽器語言（中文以外都用英文）
    pub fn saved_en() -> bool {
        match local_storage().and_then(|s| s.get_item(LANG_KEY).ok().flatten()).as_deref() {
            Some("en") => true,
            Some("zh") => false,
            _ => !web_sys::window().and_then(|w| w.navigator().language()).unwrap_or_default().to_lowercase().starts_with("zh"),
        }
    }

    fn skip(n: &Node) -> bool {
        let e = if n.node_type() == Node::ELEMENT_NODE { Some(n.clone().unchecked_into::<Element>()) } else { n.parent_element() };
        let Some(e) = e else { return false };
        if matches!(e.tag_name().as_str(), "SCRIPT" | "STYLE" | "TEXTAREA") {
            return true;
        }
        // 可編輯區（TODO 筆記）裡是使用者寫的內容
        if e.closest("[contenteditable]").ok().flatten().is_some() {
            return true;
        }
        e.closest("[data-nt]").ok().flatten().is_some()
    }

    fn translate_attrs(e: &Element) {
        // textarea 只是不翻內容（使用者打的字），placeholder 還是要翻
        if e.closest("[data-nt]").ok().flatten().is_some() {
            return;
        }
        for a in ATTRS {
            if let Some(v) = e.get_attribute(a) {
                let t = translate(&v);
                if t != v {
                    let _ = e.set_attribute(a, &t);
                }
            }
        }
    }

    fn translate_text(n: &Node) {
        if let Some(v) = n.node_value() {
            if has_cjk(&v) && !skip(n) {
                let t = translate(&v);
                if t != v {
                    n.set_node_value(Some(&t));
                }
            }
        }
    }

    fn walk(n: &Node) {
        match n.node_type() {
            Node::TEXT_NODE => translate_text(n),
            Node::ELEMENT_NODE => {
                let e: &Element = n.unchecked_ref();
                translate_attrs(e);
                if skip(n) {
                    return;
                }
                let kids = n.child_nodes();
                for i in 0..kids.length() {
                    if let Some(k) = kids.item(i) {
                        walk(&k);
                    }
                }
            }
            _ => {}
        }
    }

    /// 頁面一開始呼叫：決定語言；英文模式就翻譯整頁並持續監看
    pub fn init() {
        let en = saved_en();
        EN.with(|e| e.set(en));
        let d = doc();
        if let Some(root) = d.document_element() {
            let _ = root.set_attribute("lang", if en { "en" } else { "zh-Hant" });
        }
        if let Some(b) = d.get_element_by_id("lang-toggle") {
            b.set_text_content(Some(if en { "中文" } else { "EN" }));
            let _ = b.set_attribute("title", if en { "切換成中文" } else { "Switch to English" });
            listen(&b, "click", move |_| switch(!en));
        }
        if !en {
            return;
        }
        d.set_title(&translate(&d.title()));
        if let Some(body) = d.body() {
            walk(&body);
            let cb = Closure::<dyn FnMut(js_sys::Array)>::new(|records: js_sys::Array| {
                for r in records.iter() {
                    let r: MutationRecord = r.unchecked_into();
                    match r.type_().as_str() {
                        "childList" => {
                            let added = r.added_nodes();
                            for i in 0..added.length() {
                                if let Some(n) = added.item(i) {
                                    walk(&n);
                                }
                            }
                        }
                        "characterData" => {
                            if let Some(t) = r.target() {
                                translate_text(&t);
                            }
                        }
                        "attributes" => {
                            if let Some(t) = r.target() {
                                translate_attrs(t.unchecked_ref());
                            }
                        }
                        _ => {}
                    }
                }
            });
            let obs = MutationObserver::new(cb.as_ref().unchecked_ref()).unwrap();
            let opts = MutationObserverInit::new();
            opts.set_child_list(true);
            opts.set_subtree(true);
            opts.set_character_data(true);
            opts.set_attributes(true);
            let filter = js_sys::Array::new();
            for a in ATTRS {
                filter.push(&a.into());
            }
            opts.set_attribute_filter(&filter);
            let _ = obs.observe_with_options(&body, &opts);
            cb.forget();
        }
    }

    /// 切換語言：存起來、更新右鍵選單，然後重新載入頁面
    fn switch(en: bool) {
        let v = if en { "en" } else { "zh" };
        if let Some(s) = local_storage() {
            let _ = s.set_item(LANG_KEY, v);
        }
        crate::ui::spawn(async move {
            crate::chrome::set(&[(LANG_KEY, crate::chrome::to_js(v))]).await;
            EN.with(|e| e.set(en));
            crate::chrome::update_context_menus();
            let _ = web_sys::window().unwrap().location().reload();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_symbols() {
        assert_eq!(translate("☰ 收藏"), "☰ Saved");
        assert_eq!(translate(" 收藏 "), " Saved ");
        assert_eq!(translate("⚠ 無法移動：boom"), "⚠ Couldn't move: boom");
        assert_eq!(translate("Hello"), "Hello");
        assert_eq!(translate("沒在表裡的中文"), "沒在表裡的中文");
    }

    #[test]
    fn patterns_and_filters() {
        assert_eq!(translate("已同步 12 部 · 5 分鐘前"), "Synced 12 · 5 min ago");
        assert_eq!(translate("剩 3 項 · 已完成 1 項"), "3 left · 1 done");
        assert_eq!(translate("10月7日 週三 17:38"), "Wed, Oct 7 17:38");
        assert_eq!(translate("星期三下午3:00"), "Wed 3:00 PM");
        assert_eq!(translate("上午9時"), "9 AM");
        assert_eq!(translate("10/9 (週五) 18:00"), "Fri 10/9 18:00");
        assert_eq!(translate("今天 18:00 · 已到期"), "Today 18:00 · overdue");
        assert_eq!(translate("1 小時 5 分鐘計時（已暫停）"), "1 hr 5 min timer (paused)");
        assert_eq!(translate("5 分鐘"), "5 min");
        assert_eq!(translate("（7）"), " (7)");
        assert_eq!(translate("設為高優先"), "Set High priority");
        assert_eq!(translate("已加入 3 部，略過 2 筆（重複或無效）"), "Added 3, skipped 2 (duplicates or invalid)");
    }

    #[test]
    fn multiline() {
        assert_eq!(
            translate("Google 拒絕了登入要求，通常是 OAuth 用戶端設定不對：\n• 用戶端類型要是「網頁應用程式」"),
            "Google rejected the sign-in. Usually the OAuth client is misconfigured:\n• The client type must be “Web application”"
        );
    }

    #[test]
    fn no_duplicate_keys() {
        let mut seen = std::collections::HashSet::new();
        for (z, _) in EXACT {
            assert!(seen.insert(*z), "duplicate: {z}");
        }
    }
}
