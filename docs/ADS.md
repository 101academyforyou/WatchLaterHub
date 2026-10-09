# 贊助內容設定檔（ads.json）

WatchLaterHub 新分頁左側的贊助內容，由 `docs/ads.json` 決定。
這個資料夾用 GitHub Pages 發布（Settings → Pages → Branch: `main`、資料夾 `/docs`），
網址是 `https://101academyforyou.github.io/WatchLaterHub/ads.json`（擴充功能裡的設定在 `src/config.rs` 的 `ADS_URL`）。

改完 `ads.json` 推到 GitHub，GitHub Pages 約 1 分鐘更新；使用者最慢 10 分鐘內就會看到新內容，不用重新上架擴充功能。

```json
{
  "ads": [
    {
      "id": "spring-course",
      "image": "https://example.com/banner.png",
      "text": "一行中文說明（建議 30 字以內）",
      "text_en": "One line in English (optional; falls back to text)",
      "url": "https://example.com/?utm_source=watchlaterhub&utm_medium=newtab",
      "start": "2026-10-01",
      "end": "2026-10-31"
    }
  ]
}
```

- `id`：每則不同，任意英數字
- `image`：圖片網址，必須是 `https://`；建議 400×250 左右（顯示寬 200px，會自動縮放）
- `text`／`text_en`：一行文字（只顯示文字，不支援 HTML）；英文介面用 `text_en`，沒填就用 `text`
- `url`：點擊後在新分頁打開，必須是 `https://`；想知道點擊數，請在網址加上 UTM 參數，從對方網站的分析工具查看
- `start`／`end`：上架與下架日期（`YYYY-MM-DD`，含當天），留空表示不限
- 同時有多則有效的贊助內容時，每開一個新分頁隨機顯示一則
- `"ads": []` 就是不顯示任何贊助內容

注意：只放清楚、不誤導的內容，並在商店介紹中說明「會顯示贊助內容」。
