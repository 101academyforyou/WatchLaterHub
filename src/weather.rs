//! TODO 清單標題列：今天日期、天氣標籤；點天氣標籤展開 Google 風格的天氣卡片
//!
//! 天氣：Open-Meteo（免金鑰）。地名：BigDataCloud 反查（免金鑰）；城市搜尋：Open-Meteo Geocoding。
//! 位置：使用者「選擇地區」的地點；按過「使用目前位置」就用瀏覽器定位；都沒有就是預設的臺北市。結果快取 30 分鐘。

use serde::{Deserialize, Serialize};

pub const CACHE_KEY: &str = "weatherCache4";
pub const LOC_KEY: &str = "wxLocation"; // 使用者選的地區
pub const UNIT_KEY: &str = "wxUnit"; // "C" 或 "F"
const CACHE_MS: f64 = 30.0 * 60_000.0;
const FALLBACK: (f64, f64, &str) = (25.0330, 121.5654, "臺北市");
pub const GEO_KEY: &str = "wxUseGeo"; // 使用者按過「使用目前位置」
/// 圖表顯示幾個點（每 3 小時一點 → 24 小時）
pub const POINTS: usize = 8;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hour {
    /// "2026-10-02T05:00"（當地時間）
    pub t: String,
    pub temp: f64,
    pub code: u32,
    pub rain: Option<f64>,
    pub wind: f64,
    /// 風從哪個方向吹來（度，0 = 北）
    pub dir: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Day {
    /// "2026-10-02"
    pub date: String,
    pub code: u32,
    pub max: f64,
    pub min: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Forecast {
    pub at: f64,
    pub lat: f64,
    pub lon: f64,
    pub place: String,
    /// "2026-10-02T04:15"（當地時間）
    pub now: String,
    pub temp: f64,
    pub code: u32,
    pub is_day: bool,
    pub humidity: Option<f64>,
    pub wind: Option<f64>,
    pub hourly: Vec<Hour>,
    pub daily: Vec<Day>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Location {
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}

// ---------- 純邏輯（可在本機 cargo test） ----------

/// WMO 天氣代碼 → (圖示, 中文)；晚上的晴天用月亮
pub fn describe(code: u32, is_day: bool) -> (&'static str, &'static str) {
    match code {
        0 if !is_day => ("🌙", "晴朗"),
        0 => ("☀️", "晴朗"),
        1 if !is_day => ("🌙", "大致晴朗"),
        1 => ("🌤️", "大致晴朗"),
        2 => ("⛅", "多雲"),
        3 => ("☁️", "陰天"),
        45 | 48 => ("🌫️", "霧"),
        51 | 53 | 55 | 56 | 57 => ("🌦️", "毛毛雨"),
        61 | 63 | 80 | 81 => ("🌧️", "雨"),
        65 | 82 => ("🌧️", "大雨"),
        66 | 67 => ("🌧️", "凍雨"),
        71 | 73 | 75 | 77 | 85 | 86 => ("🌨️", "雪"),
        95 => ("⛈️", "雷雨"),
        96 | 99 => ("⛈️", "雷雨冰雹"),
        _ => ("🌡️", "—"),
    }
}

/// 攝氏 → 顯示用整數（°C 或 °F）
pub fn deg(c: f64, fahrenheit: bool) -> i64 {
    (if fahrenheit { c * 9.0 / 5.0 + 32.0 } else { c }).round() as i64
}

/// 星期幾（0 = 週日），Sakamoto 演算法
pub fn weekday(y: i32, m: u32, d: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + T[(m - 1) as usize] + d as i32) % 7) as u32
}

const WD: [&str; 7] = ["日", "一", "二", "三", "四", "五", "六"];

fn ymd(s: &str) -> Option<(i32, u32, u32)> {
    let mut it = s.get(..10)?.split('-');
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

fn hm(s: &str) -> Option<(u32, u32)> {
    let t = s.split('T').nth(1)?;
    Some((t.get(..2)?.parse().ok()?, t.get(3..5)?.parse().ok()?))
}

/// 「週五」
pub fn day_label(date: &str) -> String {
    ymd(date).map(|(y, m, d)| format!("週{}", WD[weekday(y, m, d) as usize])).unwrap_or_default()
}

/// 12 小時制：「上午5時」「下午2時」「上午12時」
pub fn hour_label(t: &str) -> String {
    let Some((h, _)) = hm(t) else { return String::new() };
    let ampm = if h < 12 { "上午" } else { "下午" };
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    format!("{ampm}{h12}時")
}

/// 「星期五上午4:00」（整點，跟 Google 一樣）
pub fn now_label(now: &str) -> String {
    let (Some((y, m, d)), Some((h, _))) = (ymd(now), hm(now)) else { return String::new() };
    let ampm = if h < 12 { "上午" } else { "下午" };
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    format!("星期{}{ampm}{h12}:00", WD[weekday(y, m, d) as usize])
}

/// 圖表的點：今天 → 從目前這個小時起每 3 小時；其他天 → 那天 0、3、…、21 時
pub fn chart_points(f: &Forecast, day: usize) -> Vec<&Hour> {
    if day == 0 {
        let this_hour = f.now.get(..13).unwrap_or("");
        let start = f.hourly.iter().position(|h| h.t.get(..13).unwrap_or("") >= this_hour).unwrap_or(0);
        f.hourly[start..].iter().step_by(3).take(POINTS).collect()
    } else {
        let Some(date) = f.daily.get(day).map(|d| d.date.as_str()) else { return vec![] };
        f.hourly.iter().filter(|h| h.t.starts_with(date)).step_by(3).take(POINTS).collect()
    }
}

/// 目前這個小時的降雨機率
pub fn rain_now(f: &Forecast) -> Option<f64> {
    let this_hour = f.now.get(..13)?;
    f.hourly.iter().find(|h| h.t.starts_with(this_hour)).and_then(|h| h.rain)
}

/// 解析 Open-Meteo 回應
pub fn parse(j: &serde_json::Value, at: f64, lat: f64, lon: f64, place: &str) -> Option<Forecast> {
    let c = &j["current"];
    let h = &j["hourly"];
    let d = &j["daily"];
    let hourly = h["time"]
        .as_array()?
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            Some(Hour {
                t: t.as_str()?.to_string(),
                temp: h["temperature_2m"][i].as_f64()?,
                code: h["weather_code"][i].as_u64()? as u32,
                rain: h["precipitation_probability"][i].as_f64(),
                wind: h["wind_speed_10m"][i].as_f64().unwrap_or(0.0),
                dir: h["wind_direction_10m"][i].as_f64().unwrap_or(0.0),
            })
        })
        .collect();
    let daily = d["time"]
        .as_array()?
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            Some(Day {
                date: t.as_str()?.to_string(),
                code: d["weather_code"][i].as_u64()? as u32,
                max: d["temperature_2m_max"][i].as_f64()?,
                min: d["temperature_2m_min"][i].as_f64()?,
            })
        })
        .collect();
    Some(Forecast {
        at,
        lat,
        lon,
        place: place.to_string(),
        now: c["time"].as_str()?.to_string(),
        temp: c["temperature_2m"].as_f64()?,
        code: c["weather_code"].as_u64()? as u32,
        is_day: c["is_day"].as_u64().unwrap_or(1) == 1,
        humidity: c["relative_humidity_2m"].as_f64(),
        wind: c["wind_speed_10m"].as_f64(),
        hourly,
        daily,
    })
}

/// 標題列上的天氣標籤：「🌙 26° 晴朗」
pub fn short_text(f: &Forecast, fahrenheit: bool) -> String {
    let (icon, text) = describe(f.code, f.is_day);
    format!("{icon} {}° {text}", deg(f.temp, fahrenheit))
}

/// 反查地名：「新北市中和區」
pub fn place_name(j: &serde_json::Value) -> Option<String> {
    let s = |k: &str| j[k].as_str().map(str::trim).filter(|x| !x.is_empty()).map(String::from);
    let region = s("principalSubdivision");
    let local = s("city").or_else(|| s("locality"));
    match (region, local) {
        (Some(r), Some(l)) if l.contains(&r) => Some(l),
        (Some(r), Some(l)) if r.contains(&l) => Some(r),
        (Some(r), Some(l)) => Some(format!("{r}{l}")),
        (r, l) => r.or(l),
    }
}

// ---------- 城市搜尋 ----------

/// 搜尋結果：地名（存成地點名稱）、副標（省／國家）
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub sub: String,
    pub lat: f64,
    pub lon: f64,
}

/// 同一個地名的幾種寫法：台↔臺、加不加「市」
pub fn query_variants(q: &str) -> Vec<String> {
    let q = q.trim();
    let mut v = vec![q.to_string()];
    let alt = if q.contains('台') { q.replace('台', "臺") } else { q.replace('臺', "台") };
    v.push(alt);
    for base in v.clone() {
        if !base.ends_with('市') && !base.ends_with('縣') && base.chars().count() <= 3 && !base.is_ascii() {
            v.push(format!("{base}市"));
        }
    }
    v.dedup();
    let mut out: Vec<String> = vec![];
    for x in v {
        if !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

/// 兩個名稱若一個包含另一個，取長的（「東京」+「東京都」→「東京都」）；否則接起來
fn merge_names(a: &str, b: &str) -> String {
    if a.is_empty() || b.contains(a) {
        b.to_string()
    } else if b.is_empty() || a.contains(b) {
        a.to_string()
    } else {
        format!("{a}{b}")
    }
}

/// Open-Meteo Geocoding 的結果
/// 行政區名稱清理：「A or B」只取 A；「臺灣省／台灣省」這種省級名稱不顯示
pub fn clean_admin(s: &str) -> Option<&str> {
    let s = s.split(" or ").next().unwrap_or("").trim();
    let lower = s.to_lowercase();
    let province = ["臺灣省", "台灣省", "台湾省", "taiwan province", "province of taiwan"];
    (!s.is_empty() && !province.iter().any(|p| lower == *p)).then_some(s)
}

/// 國名統一：台湾／台灣／Taiwan → 臺灣
pub fn clean_country(s: &str) -> String {
    match s.trim() {
        "台湾" | "台灣" | "臺灣" | "Taiwan" | "中華民國" | "中華民國（臺灣）" => "臺灣".to_string(),
        x => x.to_string(),
    }
}

pub fn places_open_meteo(j: &serde_json::Value) -> Vec<Place> {
    j["results"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    let name = clean_admin(r["name"].as_str()?)?.to_string();
                    // 省級（臺灣省）去掉後，改用下一層的縣市
                    let admin1 = r["admin1"].as_str().and_then(clean_admin).or_else(|| r["admin2"].as_str().and_then(clean_admin)).unwrap_or("");
                    let country = clean_country(r["country"].as_str().unwrap_or(""));
                    let country = country.as_str();
                    let name = merge_names(admin1, &name);
                    let sub = [admin1, country].iter().filter(|x| !x.is_empty() && !name.contains(*x)).cloned().collect::<Vec<_>>().join(", ");
                    Some(Place { name, sub, lat: r["latitude"].as_f64()?, lon: r["longitude"].as_f64()? })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
}

/// OpenStreetMap Nominatim 的結果（需要 addressdetails=1）。
/// 只取到「縣市＋區」這一層：里、鄰、路名都不顯示，例如「中和里」會變成它所在的「新北市中和區」或「新北市淡水區」
pub fn places_nominatim(j: &serde_json::Value) -> Vec<Place> {
    let Some(arr) = j.as_array() else { return vec![] };
    arr.iter()
        .filter_map(|r| {
            let a = &r["address"];
            let get = |keys: &[&str]| keys.iter().find_map(|k| a[*k].as_str().and_then(clean_admin));
            let region = get(&["state", "city", "county", "province"]);
            let city = get(&["city", "county"]).filter(|c| Some(*c) != region);
            let district = get(&["city_district", "district", "suburb", "town", "borough", "municipality"]);
            let mut levels: Vec<&str> = vec![];
            for x in [region, city, district].into_iter().flatten() {
                if !levels.iter().any(|l| l.contains(x) || x.contains(l)) {
                    levels.push(x);
                }
            }
            let country = clean_country(get(&["country"]).unwrap_or(""));
            let (name, sub) = if levels.is_empty() {
                // 沒有地址細節：只能用名稱（國家層級等）
                (r["name"].as_str().filter(|x| !x.is_empty())?.to_string(), country)
            } else if levels.iter().any(|l| has_cjk(l)) {
                // 中文地名直接連起來：「新北市中和區」
                (levels.iter().rev().take(2).rev().cloned().collect::<String>(), country)
            } else {
                // 英文地名：最小的一層當名稱，其餘放後面
                let last = levels.pop().unwrap_or_default().to_string();
                let mut rest: Vec<String> = levels.iter().rev().map(|x| x.to_string()).collect();
                if !country.is_empty() {
                    rest.push(country);
                }
                (last, rest.join(", "))
            };
            Some(Place { name, sub, lat: r["lat"].as_str()?.parse().ok()?, lon: r["lon"].as_str()?.parse().ok()? })
        })
        .collect()
}

/// 比對用：去空白、臺→台、轉小寫
fn norm(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).map(|c| if c == '臺' { '台' } else { c }).collect::<String>().to_lowercase()
}

/// 中文查詢：查詢的每個字都要依序出現在地名裡（「新北中和」→「新北市中和區」符合，「新北市淡水區」不符合）
pub fn matches_query(query: &str, name: &str) -> bool {
    let q = norm(query);
    if !has_cjk(&q) {
        return true; // 英文等查詢交給搜尋服務判斷
    }
    let n = norm(name);
    let mut it = n.chars();
    q.chars().all(|c| it.any(|x| x == c))
}

/// 合併多個來源：去掉跟查詢對不上的、同名或位置幾乎相同的重複結果，最多 8 筆
pub fn merge_places(query: &str, lists: Vec<Vec<Place>>) -> Vec<Place> {
    let mut out: Vec<Place> = vec![];
    for p in lists.into_iter().flatten() {
        if !matches_query(query, &p.name) {
            continue;
        }
        let dup = out.iter().any(|o| norm(&o.name) == norm(&p.name) || ((o.lat - p.lat).abs() < 0.03 && (o.lon - p.lon).abs() < 0.03));
        if !dup {
            out.push(p);
        }
        if out.len() == 8 {
            break;
        }
    }
    out
}

/// 「10月2日 週五 04:55」（24 小時制）
pub fn date_text(month0: u32, day: u32, weekday: u32, hour: u32, minute: u32) -> String {
    format!("{}月{}日 週{} {hour:02}:{minute:02}", month0 + 1, day, WD[(weekday % 7) as usize])
}

// ---------- 圖表（SVG） ----------

pub fn chart_svg(points: &[&Hour], tab: &str, fahrenheit: bool) -> String {
    const W: f64 = 480.0;
    let n = points.len().max(1) as f64;
    let step = W / n;
    let x = |i: usize| step * (i as f64 + 0.5);
    let mut s = String::new();
    match tab {
        "rain" => {
            for (i, p) in points.iter().enumerate() {
                let r = p.rain.unwrap_or(0.0).clamp(0.0, 100.0);
                let hgt = (r / 100.0 * 70.0).max(1.5);
                s.push_str(&format!(
                    r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="3" class="wx-bar"/><text x="{:.1}" y="{:.1}" class="wx-lbl">{}%</text>"#,
                    x(i) - step * 0.3, 92.0 - hgt, step * 0.6, hgt, x(i), 86.0 - hgt, r.round()
                ));
            }
        }
        "wind" => {
            for (i, p) in points.iter().enumerate() {
                // 箭頭指向風吹去的方向
                s.push_str(&format!(
                    r#"<g transform="translate({:.1},58) rotate({:.0})"><path d="M0,-14 L7,6 L0,1 L-7,6 Z" class="wx-arrow"/></g><text x="{:.1}" y="96" class="wx-lbl">{} 公里/時</text>"#,
                    x(i), p.dir + 180.0, x(i), p.wind.round()
                ));
            }
        }
        _ => {
            let temps: Vec<f64> = points.iter().map(|p| p.temp).collect();
            let lo = temps.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let span = (hi - lo).max(4.0);
            let y = |t: f64| 90.0 - (t - lo) / span * 55.0;
            let pts: Vec<String> = temps.iter().enumerate().map(|(i, t)| format!("{:.1},{:.1}", x(i), y(*t))).collect();
            if !pts.is_empty() {
                s.push_str(&format!(r#"<polygon points="0,{:.1} {} {W},{:.1} {W},110 0,110" class="wx-area"/>"#, y(temps[0]), pts.join(" "), y(*temps.last().unwrap())));
                s.push_str(&format!(r#"<polyline points="0,{:.1} {} {W},{:.1}" class="wx-line"/>"#, y(temps[0]), pts.join(" "), y(*temps.last().unwrap())));
            }
            for (i, t) in temps.iter().enumerate() {
                let cls = if i == 0 { "wx-lbl first" } else { "wx-lbl" };
                s.push_str(&format!(r#"<text x="{:.1}" y="{:.1}" class="{cls}">{}</text>"#, x(i), y(*t) - 8.0, deg(*t, fahrenheit)));
            }
        }
    }
    // 時間軸
    for (i, p) in points.iter().enumerate() {
        s.push_str(&format!(r#"<text x="{:.1}" y="128" class="wx-axis">{}</text>"#, x(i), hour_label(&p.t)));
    }
    format!(r#"<svg viewBox="0 0 {W} 134" class="wx-svg" role="img" aria-label="逐時預報">{s}</svg>"#)
}

// ---------- 畫面 ----------

mod view {
    use super::*;
    use crate::chrome::{self, to_js};
    use crate::ui::{doc, el, hide, listen, on_click, spawn};
    use std::cell::RefCell;
    use wasm_bindgen::JsCast;
    use web_sys::{Element, HtmlInputElement, KeyboardEvent};

    struct State {
        data: Option<Forecast>,
        day: usize,
        tab: String,
        fahrenheit: bool,
    }

    thread_local! {
        static S: RefCell<State> = RefCell::new(State { data: None, day: 0, tab: "temp".into(), fahrenheit: false });
    }

    fn render_date() {
        let d = js_sys::Date::new_0();
        el("todo-date").set_text_content(Some(&date_text(d.get_month(), d.get_date(), d.get_day(), d.get_hours(), d.get_minutes())));
    }

    /// 每分鐘整點更新時間
    fn tick_clock() {
        render_date();
        let ms_to_next = 60_000 - (js_sys::Date::now() as i64 % 60_000) as i32 + 50;
        crate::ui::timeout(ms_to_next, tick_clock);
    }

    fn set_text(id: &str, t: &str) {
        el(id).set_text_content(Some(t));
    }

    /// 重畫標題列標籤與卡片
    fn render() {
        S.with(|s| {
            let s = s.borrow();
            let Some(f) = &s.data else { return };
            let fh = s.fahrenheit;
            let chip = el("todo-weather");
            chip.set_text_content(Some(&short_text(f, fh)));
            chip.set_title("點一下查看詳細天氣");
            hide("todo-weather", false);

            set_text("wx-place", &f.place);
            let (icon, cond) = describe(f.code, f.is_day);
            set_text("wx-icon", icon);
            set_text("wx-t", &deg(f.temp, fh).to_string());
            let _ = el("wx-c").class_list().toggle_with_force("on", !fh);
            let _ = el("wx-f").class_list().toggle_with_force("on", fh);
            let meta = [
                format!("降雨機率：{}%", rain_now(f).map(|r| r.round()).unwrap_or(0.0)),
                format!("濕度：{}%", f.humidity.map(|h| h.round()).unwrap_or(0.0)),
                format!("風速：{} 公里/時", f.wind.map(|w| w.round()).unwrap_or(0.0)),
            ];
            el("wx-meta").set_inner_html("");
            for m in meta {
                let div = doc().create_element("div").unwrap();
                div.set_text_content(Some(&m));
                el("wx-meta").append_child(&div).unwrap();
            }
            // 右側：選中那天的資訊
            let (when, cond_txt) = if s.day == 0 {
                (now_label(&f.now), cond.to_string())
            } else {
                let d = &f.daily[s.day];
                (format!("星期{}", day_label(&d.date).trim_start_matches('週')), describe(d.code, true).1.to_string())
            };
            set_text("wx-time", &when);
            set_text("wx-cond", &cond_txt);

            // 分頁
            if let Ok(tabs) = doc().query_selector_all(".wx-tabs button") {
                for i in 0..tabs.length() {
                    let b: Element = tabs.item(i).unwrap().unchecked_into();
                    let on = b.get_attribute("data-tab").as_deref() == Some(s.tab.as_str());
                    let _ = b.class_list().toggle_with_force("on", on);
                    let _ = b.set_attribute("aria-selected", if on { "true" } else { "false" });
                }
            }
            el("wx-chart").set_inner_html(&chart_svg(&chart_points(f, s.day), &s.tab, fh));

            // 8 天預報
            let days = el("wx-days");
            days.set_inner_html("");
            for (i, d) in f.daily.iter().enumerate() {
                let b = doc().create_element("button").unwrap();
                b.set_class_name(if i == s.day { "wx-day on" } else { "wx-day" });
                let _ = b.set_attribute("type", "button");
                b.set_inner_html(r#"<span class="wd"></span><span class="wi"></span><span class="wt"><b></b> <span></span></span>"#);
                let q = |sel: &str| b.query_selector(sel).ok().flatten().unwrap();
                q(".wd").set_text_content(Some(&day_label(&d.date)));
                q(".wi").set_text_content(Some(describe(d.code, true).0));
                let _ = b.set_attribute("title", describe(d.code, true).1);
                q(".wt b").set_text_content(Some(&format!("{}°", deg(d.max, fh))));
                q(".wt span").set_text_content(Some(&format!("{}°", deg(d.min, fh))));
                listen(&b, "click", move |e| {
                    e.stop_propagation();
                    S.with(|s| s.borrow_mut().day = i);
                    render();
                });
                days.append_child(&b).unwrap();
            }
        });
    }

    // ---------- 資料 ----------

    async fn locate() -> (f64, f64, String) {
        if let Some(l) = chrome::get::<Location>(LOC_KEY).await {
            return (l.lat, l.lon, l.name);
        }
        // 預設：臺北市（按過「使用目前位置」才用定位）
        if !chrome::get_or(GEO_KEY, false).await {
            return (FALLBACK.0, FALLBACK.1, FALLBACK.2.to_string());
        }
        match chrome::current_position().await {
            Some((lat, lon)) => {
                let url = format!(
                    "https://api.bigdatacloud.net/data/reverse-geocode-client?latitude={lat:.4}&longitude={lon:.4}&localityLanguage=zh-Hant"
                );
                let name = match chrome::fetch(&url, None, "GET").await {
                    Ok(r) if r.status < 400 => place_name(&r.json),
                    _ => None,
                };
                (lat, lon, name.unwrap_or_else(|| "目前位置".into()))
            }
            None => (FALLBACK.0, FALLBACK.1, format!("{}（取不到你的位置）", FALLBACK.2)),
        }
    }

    async fn fetch_forecast() -> Option<Forecast> {
        let (lat, lon, place) = locate().await;
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}\
             &current=temperature_2m,weather_code,relative_humidity_2m,wind_speed_10m,is_day\
             &hourly=temperature_2m,weather_code,precipitation_probability,wind_speed_10m,wind_direction_10m\
             &daily=weather_code,temperature_2m_max,temperature_2m_min\
             &timezone=auto&forecast_days=8"
        );
        let r = chrome::fetch(&url, None, "GET").await.ok()?;
        if r.status >= 400 {
            return None;
        }
        parse(&r.json, chrome::now(), lat, lon, &place)
    }

    async fn load(force: bool) {
        let cached: Option<Forecast> = chrome::get(CACHE_KEY).await;
        if let Some(f) = &cached {
            S.with(|s| s.borrow_mut().data = Some(f.clone()));
            render();
            if !force && chrome::now() - f.at < CACHE_MS {
                return;
            }
        }
        match fetch_forecast().await {
            Some(f) => {
                chrome::set(&[(CACHE_KEY, to_js(&f))]).await;
                S.with(|s| {
                    let mut s = s.borrow_mut();
                    s.data = Some(f);
                    s.day = 0;
                });
                render();
            }
            None if cached.is_none() => hide("todo-weather", true),
            None => {}
        }
    }

    /// TODO 清單打開時呼叫：日期馬上更新；天氣用快取，過期才重抓
    pub fn refresh() {
        render_date();
        spawn(async {
            let unit: String = chrome::get_or(UNIT_KEY, "C".to_string()).await;
            S.with(|s| s.borrow_mut().fahrenheit = unit == "F");
            load(false).await;
        });
    }

    // ---------- 選擇地區 ----------

    async fn search_city() {
        let q = el("wx-city").unchecked_into::<HtmlInputElement>().value().trim().to_string();
        let list = el("wx-results");
        list.set_inner_html("");
        if q.is_empty() {
            return;
        }
        set_text("wx-pick-msg", "");
        list.set_inner_html(r#"<div class="wx-res-none">搜尋中…</div>"#);
        // Open-Meteo（多試幾種寫法）＋ OpenStreetMap，一起查
        let mut lists = vec![];
        let osm = format!(
            "https://nominatim.openstreetmap.org/search?q={}&format=json&limit=15&addressdetails=1&accept-language=zh-TW",
            js_sys::encode_uri_component(&q)
        );
        if let Ok(r) = chrome::fetch(&osm, None, "GET").await {
            if r.status < 400 {
                lists.push(places_nominatim(&r.json));
            }
        }
        for v in query_variants(&q) {
            let url = format!(
                "https://geocoding-api.open-meteo.com/v1/search?name={}&count=8&language=zh&format=json",
                js_sys::encode_uri_component(&v)
            );
            if let Ok(r) = chrome::fetch(&url, None, "GET").await {
                lists.push(places_open_meteo(&r.json));
            }
        }
        let places = merge_places(&q, lists);
        list.set_inner_html("");
        if places.is_empty() {
            list.set_inner_html(r#"<div class="wx-res-none">找不到這個地方，換個名稱試試（例如：臺中、Taichung、東京）</div>"#);
            return;
        }
        for p in places {
            let b = doc().create_element("button").unwrap();
            b.set_class_name("wx-res");
            let _ = b.set_attribute("type", "button");
            b.set_inner_html(r#"<b></b><span></span>"#);
            b.query_selector("b").ok().flatten().unwrap().set_text_content(Some(&p.name));
            // 「新北市中和區, 臺灣」
            b.query_selector("span").ok().flatten().unwrap().set_text_content(Some(&if p.sub.is_empty() { String::new() } else { format!(", {}", p.sub) }));
            listen(&b, "click", move |e| {
                e.stop_propagation();
                let loc = Location { lat: p.lat, lon: p.lon, name: p.name.clone() };
                spawn(async move {
                    chrome::set(&[(LOC_KEY, to_js(&loc)), (GEO_KEY, to_js(&false))]).await;
                    el("wx-results").set_inner_html("");
                    hide("wx-pick-row", true);
                    load(true).await;
                });
            });
            list.append_child(&b).unwrap();
        }
    }

    pub fn start() {
        tick_clock();
        // 點標題列的天氣標籤：展開／收起天氣卡片
        on_click("todo-weather", || {
            let open = el("wx-card").hidden();
            hide("wx-card", !open);
            let _ = el("todo-weather").class_list().toggle_with_force("on", open);
        });
        on_click("wx-c", || {
            S.with(|s| s.borrow_mut().fahrenheit = false);
            render();
            spawn(async { chrome::set(&[(UNIT_KEY, to_js("C"))]).await });
        });
        on_click("wx-f", || {
            S.with(|s| s.borrow_mut().fahrenheit = true);
            render();
            spawn(async { chrome::set(&[(UNIT_KEY, to_js("F"))]).await });
        });
        if let Ok(tabs) = doc().query_selector_all(".wx-tabs button") {
            for i in 0..tabs.length() {
                let b: Element = tabs.item(i).unwrap().unchecked_into();
                let tab = b.get_attribute("data-tab").unwrap_or_default();
                listen(&b, "click", move |_| {
                    S.with(|s| s.borrow_mut().tab = tab.clone());
                    render();
                });
            }
        }
        on_click("wx-pick", || {
            let show = el("wx-pick-row").hidden();
            hide("wx-pick-row", !show);
            set_text("wx-pick-msg", "");
            el("wx-results").set_inner_html("");
            if show {
                let _ = el("wx-city").focus();
            }
        });
        on_click("wx-city-go", || spawn(search_city()));
        listen(&el("wx-city"), "keydown", |e| {
            let k: &KeyboardEvent = e.unchecked_ref();
            if k.key() == "Enter" && !k.is_composing() {
                e.prevent_default();
                spawn(search_city());
            }
        });
        on_click("wx-here", || {
            spawn(async {
                chrome::remove(&[LOC_KEY]).await;
                chrome::set(&[(GEO_KEY, to_js(&true))]).await;
                el("wx-results").set_inner_html("");
                hide("wx-pick-row", true);
                load(true).await;
            })
        });
    }
}

pub use view::{refresh, start};

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> serde_json::Value {
        let times: Vec<String> = (0..48).map(|i| format!("2026-10-{:02}T{:02}:00", 2 + i / 24, i % 24)).collect();
        serde_json::json!({
            "current": { "time": "2026-10-02T04:15", "temperature_2m": 26.2, "weather_code": 0, "is_day": 0,
                         "relative_humidity_2m": 85, "wind_speed_10m": 6.4 },
            "hourly": {
                "time": times,
                "temperature_2m": (0..48).map(|i| 24.0 + (i % 24) as f64 * 0.3).collect::<Vec<_>>(),
                "weather_code": vec![0; 48],
                "precipitation_probability": (0..48).map(|i| (i * 2) as f64).collect::<Vec<_>>(),
                "wind_speed_10m": vec![6.0; 48],
                "wind_direction_10m": vec![90.0; 48]
            },
            "daily": { "time": ["2026-10-02", "2026-10-03"], "weather_code": [61, 2],
                       "temperature_2m_max": [31.2, 29.0], "temperature_2m_min": [24.1, 23.6] }
        })
    }

    #[test]
    fn parse_and_labels() {
        let f = parse(&sample(), 1.0, 25.0, 121.5, "新北市中和區").unwrap();
        assert_eq!(short_text(&f, false), "🌙 26° 晴朗");
        assert_eq!(short_text(&f, true), "🌙 79° 晴朗");
        assert_eq!(now_label(&f.now), "星期五上午4:00");
        assert_eq!(rain_now(&f), Some(8.0));
        assert_eq!(f.daily.len(), 2);
        assert_eq!(day_label("2026-10-03"), "週六");
        assert_eq!((hour_label("2026-10-02T00:00"), hour_label("2026-10-02T05:00"), hour_label("2026-10-02T14:00"), hour_label("2026-10-02T12:00")),
                   ("上午12時".into(), "上午5時".into(), "下午2時".into(), "下午12時".into()));
        assert_eq!(date_text(9, 2, 5, 4, 5), "10月2日 週五 04:05");
        assert_eq!(date_text(9, 2, 5, 23, 59), "10月2日 週五 23:59");
        assert!(parse(&serde_json::json!({}), 1.0, 0.0, 0.0, "x").is_none());
    }

    #[test]
    fn chart_ranges() {
        let f = parse(&sample(), 1.0, 25.0, 121.5, "x").unwrap();
        let today: Vec<&str> = chart_points(&f, 0).iter().map(|h| h.t.as_str()).collect();
        assert_eq!(today.len(), POINTS);
        assert_eq!(today[0], "2026-10-02T04:00");
        assert_eq!(today[1], "2026-10-02T07:00");
        assert_eq!(today[7], "2026-10-03T01:00");
        let tomorrow: Vec<&str> = chart_points(&f, 1).iter().map(|h| h.t.as_str()).collect();
        assert_eq!(tomorrow.first().copied(), Some("2026-10-03T00:00"));
        assert_eq!(tomorrow.last().copied(), Some("2026-10-03T21:00"));
        assert!(chart_points(&f, 9).is_empty());
        for tab in ["temp", "rain", "wind"] {
            let svg = chart_svg(&chart_points(&f, 0), tab, false);
            assert!(svg.starts_with("<svg") && svg.contains("上午4時"));
        }
    }

    #[test]
    fn city_search() {
        assert_eq!(query_variants("台中"), ["台中", "臺中", "台中市", "臺中市"]);
        assert_eq!(query_variants("Tokyo"), ["Tokyo"]);
        assert_eq!(query_variants("臺中市"), ["臺中市", "台中市"]);
        let om = places_open_meteo(&serde_json::json!({"results": [
            {"name": "東京", "admin1": "東京都", "country": "日本", "latitude": 35.69, "longitude": 139.69},
            {"name": "臺中市", "admin1": "臺中市", "country": "臺灣", "latitude": 24.15, "longitude": 120.68}
        ]}));
        assert_eq!((om[0].name.as_str(), om[0].sub.as_str()), ("東京都", "日本"));
        assert_eq!((om[1].name.as_str(), om[1].sub.as_str()), ("臺中市", "臺灣"));
        // 使用者遇到的情況：搜「新北市中和」，OSM 回傳好幾個「中和里」
        let osm = places_nominatim(&serde_json::json!([
            {"name": "中和區", "lat": "24.9985", "lon": "121.4950",
             "address": {"suburb": "中和區", "city": "新北市", "country": "臺灣"}},
            {"name": "中和", "lat": "24.9970", "lon": "121.4880",
             "address": {"neighbourhood": "枋寮里", "suburb": "中和區", "city": "新北市", "country": "臺灣"}},
            {"name": "中和", "lat": "24.9950", "lon": "121.4300",
             "address": {"village": "山佳里", "suburb": "樹林區", "city": "新北市", "country": "臺灣"}},
            {"name": "中和里", "lat": "25.1700", "lon": "121.4400",
             "address": {"village": "中和里", "suburb": "淡水區", "city": "新北市", "country": "臺灣"}},
            {"name": "臺中市", "lat": "24.1477", "lon": "120.6736", "address": {"city": "臺中市", "country": "臺灣"}},
            {"name": "San Jose", "lat": "37.33", "lon": "-121.89",
             "address": {"city": "San Jose", "county": "Santa Clara County", "state": "California", "country": "United States"}}
        ]));
        let names: Vec<String> = osm.iter().map(|p| format!("{}|{}", p.name, p.sub)).collect();
        assert_eq!(names[0], "新北市中和區|臺灣");
        assert_eq!(names[1], "新北市中和區|臺灣");
        assert_eq!(names[2], "新北市樹林區|臺灣");
        assert_eq!(names[3], "新北市淡水區|臺灣");
        assert_eq!(names[4], "臺中市|臺灣");
        assert_eq!(names[5], "San Jose|California, United States");
        // 只留跟查詢對得上的，同名去重
        let merged = merge_places("新北市中和", vec![osm.clone()]);
        assert_eq!(merged.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["新北市中和區"]);
        assert!(matches_query("新北中和", "新北市中和區"));
        // 使用者遇到的：Open-Meteo 的省級欄位是「臺灣省 or 台灣省」、國名是簡體「台湾」
        let gg = places_open_meteo(&serde_json::json!({"results": [
            {"name": "公館", "admin1": "臺灣省 or 台灣省", "admin2": "苗栗縣", "country": "台湾", "latitude": 24.5, "longitude": 120.8},
            {"name": "公館", "admin1": "臺灣省 or 台灣省", "country": "台湾", "latitude": 24.0, "longitude": 121.0}
        ]}));
        assert_eq!((gg[0].name.as_str(), gg[0].sub.as_str()), ("苗栗縣公館", "臺灣"));
        assert_eq!((gg[1].name.as_str(), gg[1].sub.as_str()), ("公館", "臺灣"));
        let n2 = places_nominatim(&serde_json::json!([
            {"name": "公館鄉", "lat": "24.5", "lon": "120.8", "address": {"town": "公館鄉", "county": "苗栗縣", "state": "臺灣省", "country": "台灣"}}
        ]));
        assert_eq!((n2[0].name.as_str(), n2[0].sub.as_str()), ("苗栗縣公館鄉", "臺灣"));
        assert_eq!(clean_admin("臺灣省 or 台灣省"), None);
        assert_eq!(clean_admin("Taiwan Province"), None);
        assert_eq!(clean_admin("新北市"), Some("新北市"));
        assert!(matches_query("台中", "臺中市"));
        assert!(matches_query("Tokyo", "東京都"));
        assert!(!matches_query("新北市中和", "新北市淡水區"));
        let merged = merge_places("台中", vec![osm, om]);
        assert_eq!(merged.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["臺中市"]);
    }

    #[test]
    fn misc() {
        assert_eq!(weekday(2026, 10, 2), 5);
        assert_eq!(weekday(2000, 1, 1), 6);
        assert_eq!(deg(26.2, true), 79);
        assert_eq!(describe(0, true).0, "☀️");
        assert_eq!(
            place_name(&serde_json::json!({"principalSubdivision": "新北市", "city": "中和區"})),
            Some("新北市中和區".into())
        );
        assert_eq!(place_name(&serde_json::json!({"principalSubdivision": "臺北市", "city": "臺北市"})), Some("臺北市".into()));
        assert_eq!(place_name(&serde_json::json!({})), None);
    }
}
