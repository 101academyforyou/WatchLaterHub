# WatchLaterHub

Chrome 新分頁：Google 風格搜尋框，上方隨機顯示一部你在 YouTube 收藏的影片。
**用 Rust 撰寫，編譯成 WebAssembly 執行。**

- **Continue with Google**：選任何 Google 帳號登入，自動載入按讚的影片與播放清單，每天同步
- 預設就有 3 部收藏影片，不登入也能用
- 「管理收藏」貼網址手動加入，或在 YouTube 上按右鍵 →「加入 WatchLaterHub」
- 「管理收藏」貼上**播放清單網址** →「取出連結」：一次列出清單裡所有影片連結，可複製或直接加入（公開／不公開清單皆可，不需登入、不耗 API 配額）
- Google 搜尋、好手氣、AI 模式

> 「稍後觀看」清單 YouTube 官方 API 不開放讀取，無法同步。

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
3. API 配額每日 10,000 單位由所有使用者共用，每天同步一次約可支撐數百人
