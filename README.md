# WatchLaterHub

Chrome 新分頁：Google 風格搜尋框，上方隨機顯示一部你在 YouTube 收藏的影片，點影片會在新分頁播放。
**用 Rust 撰寫，編譯成 WebAssembly 執行。**

- **Continue with Google**：選任何 Google 帳號登入，再勾選要自動同步載入的播放清單（含「稍後觀看」），勾選後立即同步，之後每次打開「收藏」也會自動同步；同步的影片也會列在「收藏」下方的清單
- 預設就有 3 部收藏影片，不登入也能用
- 「收藏」視窗標題旁有「YouTube」按鈕可直接開 YouTube
- 「收藏」貼網址手動加入，或在 YouTube 上按右鍵 →「加入 WatchLaterHub」
- 「收藏」貼上**播放清單網址** →「取出連結」：一次列出清單裡所有影片連結，可複製或直接加入（公開／不公開清單皆可，不需登入、不耗 API 配額）
- Google 搜尋、好手氣、AI 模式
- **TODO 提醒**：每項可按 ⏰ 設定提醒時間（5／10／30 分鐘後、明天 09:00／18:00，或直接輸入「2026/10/02 20:30」這類日期時間，也可按 📅 從月曆選），時間到跳出提醒小視窗（沒開新分頁也會跳），可按「完成」、「稍後再提醒」（5／10／30 分鐘、1 小時）或打開 TODO 清單
- **拖曳排序**：收藏的影片、TODO 待辦、書籤都可以按住拖曳調整順序；書籤還能拖進資料夾或在資料夾之間搬移
- **任務矩陣**：TODO 標題列「✦ 任務矩陣」打開緊急／重要 2×2 視窗；未分類的待辦拖進象限、象限之間互拖、直接在象限新增；TODO 清單會標示所屬象限
- **英文版**：頂列右邊的「EN／中文」切換語言（第一次會依瀏覽器語言自動選擇），右鍵選單、提醒小視窗、天氣地名也會跟著換
- **開新分頁就打開 TODO**：每開一個新分頁就只打開 TODO（其他面板關閉），游標仍在 Google 搜尋框
- **TODO 筆記**：每項待辦的 📝 打開小視窗，可改主題、寫更多筆記；圖片和任何檔案直接放在筆記裡游標的位置（貼上、按「上傳檔案」或拖進來），文字、圖片、附件混排；點附件時 PDF、圖片、影音、文字檔在新分頁打開，其他類型（Word、Excel、壓縮檔…）直接下載（單檔上限 30 MB）；點一下圖片可選 25%／50%／75%／100%／原始大小，或拖右下角自由調整大小，點兩下放大檢視；太大的圖會自動縮小
- **TODO 清單**：標題列顯示今天日期、星期幾、時間（24 小時制）與天氣；點日期打開計時器：倒數計時（1／5／10／25 分鐘或自訂，如 15、2.5、1:30，可暫停、取消；時間到跳出提醒小視窗並響鈴，關掉新分頁也會提醒）與碼表（開始／暫停／計次／重設，每筆計次可寫筆記、✕ 移除，計次可匯出 CSV，關掉新分頁也繼續計時）；點天氣標籤展開 Google 風格天氣卡片（地點與「選擇地區」、°C／°F、降雨機率／濕度／風速、氣溫／降雨機率／風向風速圖表、8 天預報，點某天看那天的逐時）（Open-Meteo、BigDataCloud、OpenStreetMap，免金鑰；「選擇地區」搜尋後點選結果；預設臺北市，按「使用目前位置」改用瀏覽器定位，30 分鐘更新一次）；頂列「✓ TODO」打開，輸入後按 Enter 加入；勾選完成、優先順序（🔴 高／🟡 中／🟢 低）、點兩下編輯、✕ 刪除、一鍵清除已完成、匯出 CSV（可用 Excel 開啟）；按鈕上顯示剩餘項數，所有分頁同步
- 收藏、TODO、書籤、最近一次只會開一個：打開其中一個，其他的自動關閉
- **搜尋**：TODO、書籤、最近都有搜尋框，打字即時篩選（多個關鍵字以空白分隔），Esc 清空
- **最近**：頂列「💧 最近」打開跟書籤一樣的右側面板，列出最近瀏覽的網頁（右邊顯示幾分鐘前；滑鼠移上去可 ✎ 改名稱、☆ 加入書籤、✕ 移除）（來自 Chrome 瀏覽紀錄，最多 30 筆；「今天／昨天／前天／本週／自訂（24 小時制，可選到分鐘）」可篩選期間，只列出在那段時間內真的瀏覽過的網頁），可點開、拖曳排序、☆ 加入書籤（已是書籤顯示 ★）、✕ 從清單移除（不會刪除瀏覽紀錄）
- **書籤列**：頂列左側跟 Chrome 一樣顯示書籤列的書籤，資料夾點開是下拉選單（可一層層點進子資料夾），放不下的收進「»」；可拖曳調整順序，也能跟右側書籤面板互相拖曳；按右鍵編輯書籤（資料夾則是改名稱）
- **工具**：頂列「⏺︎ 工具」加入電腦上的軟體，點一下就開啟（需安裝一次電腦小幫手，見下方）；快速加入 Claude、ChatGPT、Gemini、VS Code、Obsidian、Notion、Discord、Slack、Google Meet、Zoom、Teams、Spotify（有裝桌面版就開桌面版，否則用專屬網址或網頁）；搜尋框可找電腦上的其他軟體
- **右側書籤面板**：標題旁有 5 個釘選格，把書籤拖上去即可釘選、點一下開啟、✕ 取消；顯示 Chrome 書籤（書籤列、其他書籤…，資料夾可展開收合並記住狀態），在任何地方新增／修改書籤都會即時更新；右上「★ 書籤」可開關
- 書籤面板的「☆ 加入書籤」：開啟視窗輸入名稱、網址、選資料夾（可「＋ 新資料夾」直接建立）後儲存（預先填入目前這部影片；這部影片已在書籤裡時變成編輯／移除）
- 每個書籤右方的 ✎ 可直接編輯名稱、網址與資料夾；✕ 可直接移除，6 秒內可按「復原」

> 「稍後觀看」清單 YouTube 官方 API 不開放讀取，所以改用這個 Chrome 目前登入的 YouTube 帳號讀取清單頁面：請先在同一個 Chrome 登入 youtube.com（帳號要和「稍後觀看」所屬的帳號相同）。

---

## 編譯與安裝

需要 [Rust](https://rustup.rs/)。在專案資料夾執行：

```bash
./build.sh
```

第一次會自動安裝 `wasm32-unknown-unknown` target 和 `wasm-pack`（幾分鐘），之後每次編譯約 10 秒。完成後：

1. 打開 `chrome://extensions`，開啟右上角「開發人員模式」
2. 「載入未封裝項目」→ 選 **`extension/`** 資料夾
3. 開一個新分頁

改了 Rust 程式碼後，重新執行 `./build.sh`，再到 `chrome://extensions` 按 ⟳ 重新載入。

**不想在本機裝 Rust？** 推上 GitHub，Actions（`.github/workflows/build.yml`）會自動測試、編譯，
在該次執行頁面下載 `WatchLaterHub` artifact，解壓後直接載入。

---

## 專案結構

```
src/
  lib.rs       進入點：新分頁、安裝、開機、排程、右鍵選單
  config.rs    ★ 開發者設定：Google Client ID、預設影片、同步頻率
  videos.rs    純邏輯（網址解析、去重、隨機、OAuth 解析），有單元測試
  store.rs     收藏清單（chrome.storage）
  bookmarks.rs 右側書籤面板（chrome.bookmarks）
  bookbar.rs   頂列左側的書籤列
  todo.rs      TODO 清單（純邏輯有單元測試）
  weather.rs   TODO 標題列的日期與天氣卡片（有單元測試）
  timer.rs     倒數計時與碼表（有單元測試）
  recent.rs    「最近」瀏覽清單（純邏輯有單元測試）
  drag.rs      拖曳排序共用工具
  i18n.rs      英文版：中英對照表，翻譯畫面上的文字（有單元測試）
  todo_note.rs TODO 的 📝 筆記小視窗（筆記、貼上／上傳圖片）
  apps.rs      「工具」：開啟電腦上的軟體（有單元測試）
launcher/      電腦小幫手（Native Messaging host，獨立的 Rust 專案，有單元測試）
install-launcher.sh  安裝小幫手
  reminder_page.rs  TODO 提醒小視窗（extension/reminder.html）
  playlist.rs  播放清單網址 → 所有影片連結（讀取公開頁面，有單元測試）
  youtube.rs   Google 登入（帳號選擇畫面）與 YouTube Data API 同步
  ui.rs        新分頁畫面（DOM 操作）
  chrome.rs    Chrome 擴充功能 API 的 Rust 綁定
extension/
  manifest.json  newtab.html  newtab.css
  newtab.js      3 行：載入 wasm → start_newtab()
  background.js  載入 wasm，把 Chrome 事件轉給 Rust
  pkg/           ← build.sh 產生的 .wasm 與綁定（不進版控）
```

- HTML/CSS 和兩個很短的 JS 載入檔是必要的：Chrome 只能從 HTML/JS 啟動 WebAssembly。
- `manifest.json` 的 `content_security_policy` 加了 `'wasm-unsafe-eval'`，這是 MV3 執行 wasm 的必要設定。
- 測試：`cargo test`（純邏輯在本機跑，不需要瀏覽器）

---

## Google 登入設定（開發者一次性）

`manifest.json` 含有 `key`，擴充功能 ID 固定為 `lnokcijoconplpecgocjgkhhagkdmcce`，
所以 Google Cloud 上的設定跟之前的版本**完全相同，不用重做**。

1. **啟用 API**：[YouTube Data API v3](https://console.cloud.google.com/apis/library/youtube.googleapis.com) →「啟用」
2. **OAuth 同意畫面**（[Google Auth Platform](https://console.cloud.google.com/auth/overview)）：外部；範圍加
   `youtube.readonly` 與 `userinfo.email`；測試中要把登入的帳號加進「測試使用者」
3. **OAuth 用戶端**（[建立](https://console.cloud.google.com/auth/clients/create)）：類型 **網頁應用程式**，
   已授權的重新導向 URI：
   ```
   https://lnokcijoconplpecgocjgkhhagkdmcce.chromiumapp.org/
   ```
4. 把用戶端 ID 填進 `src/config.rs` 的 `GOOGLE_CLIENT_ID`（已填入 `627463072769-…`），重新 `./build.sh`

### 開啟電腦上的軟體（電腦小幫手）

Chrome 擴充功能基於安全不能直接開啟程式，所以要裝一個小幫手（Chrome Native Messaging host，原始碼在 `launcher/`）。
在專案資料夾執行一次：

```bash
./install-launcher.sh
```

會編譯 `launcher/`、把程式放到 `~/Library/Application Support/WatchLaterHub/`（macOS），並在 Chrome 的
`NativeMessagingHosts` 資料夾寫入 `com.watchlaterhub.launcher.json`（只允許 WatchLaterHub 的擴充功能 ID 呼叫）。
小幫手只會列出與開啟電腦上已安裝的軟體（macOS 掃描 /Applications 等），不會執行其他指令。
移除：刪除上述兩個位置的檔案即可。支援 macOS 與 Linux。

`GOOGLE_CLIENT_ID` 設成空字串時，使用者第一次按 Continue with Google 會看到設定精靈，可以直接貼上。

### 常見錯誤

| 訊息 | 原因／解法 |
|---|---|
| redirect_uri_mismatch | 用戶端類型不是「網頁應用程式」，或重新導向 URI 沒填對（結尾要有 `/`）；改完等幾分鐘生效 |
| access_denied | 應用程式在測試模式，且這個帳號不在「測試使用者」中 |
| 授權已失效，請重新登入 | 使用者登出 Google 或撤銷權限 → 再按一次 Continue with Google |
| 新分頁一片空白 | 沒執行 `./build.sh`（`extension/pkg/` 不存在），或載入的是專案根目錄而不是 `extension/` |

## 公開給所有人用

1. `youtube.readonly` 是敏感範圍：超過 100 人或不想出現「未驗證」警告，需送 Google OAuth 驗證
   （隱私權政策、已驗證網域、示範影片）
2. 上架 Chrome 線上應用程式商店後會拿到新 ID，要把 `https://<新ID>.chromiumapp.org/` 加入重新導向 URI
3. API 配額每日 10,000 單位由所有使用者共用，只在使用者勾選清單或按「立即同步」時才同步，不會在背景自動同步
