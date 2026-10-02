//! 倒數計時器：點 TODO 標題列的日期展開，1／5／10／25 分鐘
//!
//! 結束時間存在 chrome.storage，並用 chrome.alarms 在背景響鈴，
//! 所以關掉新分頁也會準時跳出提醒小視窗。

use serde::{Deserialize, Serialize};

pub const KEY: &str = "timer";
pub const ALARM: &str = "timer";
pub const PRESETS: [u32; 4] = [1, 5, 10, 25];

/// 自訂時間的上限：24 小時
pub const MAX_SECS: u32 = 24 * 3600;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Timer {
    /// 設定的秒數
    pub seconds: u32,
    /// 結束時間（毫秒時間戳）；暫停時無意義
    pub end: f64,
    /// 暫停中：剩下的毫秒數
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_left: Option<f64>,
}

// ---------- 純邏輯（可在本機 cargo test） ----------

pub const SW_KEY: &str = "stopwatch";

/// 碼表：累積時間 + 目前這段的開始時間；計次存的是按下當時的總時間
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Stopwatch {
    #[serde(default)]
    pub acc: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started: Option<f64>,
    #[serde(default)]
    pub laps: Vec<f64>,
    /// 每筆計次的筆記（和 laps 同順序）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl Stopwatch {
    pub fn elapsed(&self, now: f64) -> f64 {
        self.acc + self.started.map(|s| (now - s).max(0.0)).unwrap_or(0.0)
    }
    pub fn running(&self) -> bool {
        self.started.is_some()
    }
    pub fn start(&mut self, now: f64) {
        if self.started.is_none() {
            self.started = Some(now);
        }
    }
    pub fn pause(&mut self, now: f64) {
        if self.started.is_some() {
            self.acc = self.elapsed(now);
            self.started = None;
        }
    }
    pub fn lap(&mut self, now: f64) {
        if self.running() {
            self.laps.push(self.elapsed(now));
        }
    }
    /// 移除第 `n` 次計次（從 1 開始）；它的那一段時間會併到下一次，後面的編號往前遞補
    pub fn remove_lap(&mut self, n: usize) {
        if n == 0 || n > self.laps.len() {
            return;
        }
        self.laps.remove(n - 1);
        if n <= self.notes.len() {
            self.notes.remove(n - 1);
        }
        while self.notes.last().is_some_and(|x| x.is_empty()) {
            self.notes.pop();
        }
    }

    /// 第 `n` 次計次（從 1 開始）的筆記
    pub fn note(&self, n: usize) -> &str {
        self.notes.get(n.wrapping_sub(1)).map(String::as_str).unwrap_or("")
    }
    pub fn set_note(&mut self, n: usize, text: &str) {
        if n == 0 || n > self.laps.len() {
            return;
        }
        if self.notes.len() < n {
            self.notes.resize(n, String::new());
        }
        self.notes[n - 1] = text.to_string();
        while self.notes.last().is_some_and(|x| x.is_empty()) {
            self.notes.pop();
        }
    }
    /// (第幾次, 這一段的時間, 累計時間)，最新的在前面
    pub fn lap_rows(&self) -> Vec<(usize, f64, f64)> {
        let mut prev = 0.0;
        let mut rows: Vec<_> = self
            .laps
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let row = (i + 1, t - prev, *t);
                prev = *t;
                row
            })
            .collect();
        rows.reverse();
        rows
    }
}

/// 碼表計次匯出 CSV（Excel 可直接開：BOM + CRLF），依計次順序；最後一列是目前總時間
pub fn sw_csv(sw: &Stopwatch, now: f64) -> String {
    fn cell(s: &str) -> String {
        if s.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }
    let mut out = String::from("\u{feff}計次,筆記,分段時間,累計時間\r\n");
    let mut rows = sw.lap_rows();
    rows.reverse();
    for (n, split, total) in rows {
        out.push_str(&format!("{n},{},{},{}\r\n", cell(sw.note(n)), sw_fmt(split), sw_fmt(total)));
    }
    out.push_str(&format!("總時間,,,{}\r\n", sw_fmt(sw.elapsed(now))));
    out
}

/// 碼表顯示：「00:12.34」；一小時以上「1:02:03.45」
pub fn sw_fmt(ms: f64) -> String {
    let cs = (ms.max(0.0) / 10.0).floor() as u64;
    let (h, m, s, c) = (cs / 360_000, cs / 6000 % 60, cs / 100 % 60, cs % 100);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}.{c:02}")
    } else {
        format!("{m:02}:{s:02}.{c:02}")
    }
}

impl Timer {
    pub fn start(seconds: u32, now: f64) -> Self {
        Timer { seconds, end: now + seconds as f64 * 1000.0, paused_left: None }
    }
    /// 剩下的毫秒數（不會小於 0）
    pub fn left(&self, now: f64) -> f64 {
        self.paused_left.unwrap_or(self.end - now).max(0.0)
    }
    pub fn pause(&mut self, now: f64) {
        if self.paused_left.is_none() {
            self.paused_left = Some(self.left(now));
        }
    }
    pub fn resume(&mut self, now: f64) {
        if let Some(left) = self.paused_left.take() {
            self.end = now + left;
        }
    }
    pub fn paused(&self) -> bool {
        self.paused_left.is_some()
    }
    /// 已經走完的比例 0～1
    pub fn progress(&self, now: f64) -> f64 {
        let total = (self.seconds as f64 * 1000.0).max(1.0);
        (1.0 - self.left(now) / total).clamp(0.0, 1.0)
    }
}

/// 使用者輸入的倒數時間 → 秒數。
/// 「15」「2.5」= 分鐘；「1:30」= 分:秒；「1:05:00」= 時:分:秒；也接受「90秒」「1小時」「20分」
pub fn parse_duration(input: &str) -> Option<u32> {
    let s: String = input.trim().replace('：', ":").replace(' ', "");
    if s.is_empty() {
        return None;
    }
    let secs: f64 = if s.contains(':') {
        let parts: Vec<f64> = s.split(':').map(|p| p.parse::<f64>().ok().filter(|x| *x >= 0.0)).collect::<Option<_>>()?;
        match parts.as_slice() {
            [m, sec] => m * 60.0 + sec,
            [h, m, sec] => h * 3600.0 + m * 60.0 + sec,
            _ => return None,
        }
    } else if let Some(n) = s.strip_suffix("秒").or_else(|| s.strip_suffix('s')) {
        n.parse::<f64>().ok()?
    } else if let Some(n) = s.strip_suffix("小時").or_else(|| s.strip_suffix('h')) {
        n.parse::<f64>().ok()? * 3600.0
    } else {
        let n = s.strip_suffix("分鐘").or_else(|| s.strip_suffix("分")).or_else(|| s.strip_suffix('m')).unwrap_or(&s);
        n.parse::<f64>().ok()? * 60.0
    };
    let secs = secs.round();
    (secs >= 1.0 && secs <= MAX_SECS as f64).then_some(secs as u32)
}

/// 「5 分鐘」「1 分 30 秒」「45 秒」「1 小時 30 分鐘」
pub fn duration_label(seconds: u32) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    match (h, m, s) {
        (0, 0, s) => format!("{s} 秒"),
        (0, m, 0) => format!("{m} 分鐘"),
        (0, m, s) => format!("{m} 分 {s} 秒"),
        (h, 0, 0) => format!("{h} 小時"),
        (h, m, 0) => format!("{h} 小時 {m} 分鐘"),
        (h, m, s) => format!("{h} 小時 {m} 分 {s} 秒"),
    }
}

/// 「04:59」；一小時以上「1:04:59」（無條件進位到秒，剛開始會顯示整數分鐘）
pub fn mmss(ms: f64) -> String {
    let s = (ms / 1000.0).ceil().max(0.0) as u64;
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m:02}:{sec:02}")
    }
}

// ---------- 背景：時間到 ----------

pub mod background {
    use super::*;
    use crate::chrome;

    /// 鬧鐘響了：跳出提醒小視窗
    pub async fn fire() {
        let Some(t) = chrome::get::<Timer>(KEY).await else { return };
        // 暫停中、或其實還沒到（例如剛重設過）就不跳
        if t.paused() || t.end - chrome::now() > 2_000.0 {
            return;
        }
        chrome::remove(&[KEY]).await;
        chrome::open_popup_window(&format!("reminder.html?timer={}", t.seconds), 400, 280).await;
    }

    /// 開始倒數 `seconds` 秒（提醒小視窗的「再計時」也用這個）
    pub async fn start(seconds: u32) {
        let t = Timer::start(seconds, chrome::now());
        save(&t).await;
    }

    /// 存檔並重設背景鬧鐘
    pub async fn save(t: &Timer) {
        chrome::set(&[(KEY, chrome::to_js(t))]).await;
        chrome::alarm_clear(ALARM).await;
        if !t.paused() {
            chrome::alarm_at(ALARM, t.end);
        }
    }

    pub async fn cancel() {
        chrome::remove(&[KEY]).await;
        chrome::alarm_clear(ALARM).await;
    }
}

// ---------- 畫面 ----------

mod view {
    use super::background::{cancel, save, start as start_timer};
    use super::*;
    use crate::chrome;
    use crate::ui::{el, hide, on_click, spawn};
    use std::cell::{Cell, RefCell};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    thread_local! {
        static CURRENT: RefCell<Option<Timer>> = const { RefCell::new(None) };
        static SW: RefCell<Stopwatch> = RefCell::new(Stopwatch::default());
        /// (interval id, 間隔毫秒)
        static TICKING: Cell<(i32, i32)> = const { Cell::new((0, 0)) };
    }

    // ---------- 分頁：倒數計時／碼表 ----------

    fn show_tab(sw: bool) {
        hide("tm-cd", sw);
        hide("tm-sw", !sw);
        let _ = el("tm-tab-cd").class_list().toggle_with_force("on", !sw);
        let _ = el("tm-tab-sw").class_list().toggle_with_force("on", sw);
    }

    fn render_sw() {
        let now = chrome::now();
        let sw = SW.with(|s| s.borrow().clone());
        let t = sw.elapsed(now);
        el("sw-time").set_text_content(Some(&sw_fmt(t)));
        el("sw-toggle").set_text_content(Some(if sw.running() { "⏸ 暫停" } else if t > 0.0 { "▶ 繼續" } else { "▶ 開始" }));
        let lap: web_sys::HtmlButtonElement = el("sw-lap").unchecked_into();
        lap.set_text_content(Some(if sw.running() || t == 0.0 { "計次" } else { "重設" }));
        lap.set_disabled(t == 0.0);
        // 標題列：碼表有在用就顯示
        if t > 0.0 {
            let short = sw_fmt(t);
            let short = short.rsplit_once('.').map(|(a, _)| a).unwrap_or(&short);
            el("todo-sw").set_text_content(Some(&format!("{} {short}", if sw.running() { "⏲" } else { "⏸" })));
            hide("todo-sw", false);
        } else {
            hide("todo-sw", true);
        }
        hide("sw-foot", sw.laps.is_empty());
        let ol = el("sw-laps");
        let rows = sw.lap_rows();
        if ol.child_element_count() as usize != rows.len() {
            ol.set_inner_html("");
            for (n, split, total) in rows {
                let li = crate::ui::doc().create_element("li").unwrap();
                li.set_inner_html(r#"<span class="n"></span><input class="note" type="text" placeholder="＋ 筆記" autocomplete="off" spellcheck="false"><span class="s"></span><span class="t"></span><button class="bm-del" type="button" title="移除這筆計次">✕</button>"#);
                let set = |sel: &str, v: &str| li.query_selector(sel).ok().flatten().unwrap().set_text_content(Some(v));
                let x = li.query_selector(".bm-del").ok().flatten().unwrap();
                let _ = x.set_attribute("aria-label", &format!("移除計次 {n}"));
                crate::ui::listen(&x, "click", move |e| {
                    e.stop_propagation();
                    act_sw(move |sw, _| sw.remove_lap(n));
                });
                set(".n", &format!("計次 {n}"));
                set(".s", &sw_fmt(split));
                set(".t", &sw_fmt(total));
                let inp: web_sys::HtmlInputElement = li.query_selector(".note").ok().flatten().unwrap().unchecked_into();
                inp.set_value(sw.note(n));
                let _ = inp.set_attribute("aria-label", &format!("計次 {n} 的筆記"));
                // 打字就存（不重畫清單，游標不會跳掉）
                let i2 = inp.clone();
                crate::ui::listen(&inp, "input", move |_| {
                    let v = i2.value();
                    SW.with(|s| s.borrow_mut().set_note(n, &v));
                    let sw = SW.with(|s| s.borrow().clone());
                    spawn(async move { chrome::set(&[(SW_KEY, chrome::to_js(&sw))]).await });
                });
                let i3 = inp.clone();
                crate::ui::listen(&inp, "keydown", move |e| {
                    let k: &web_sys::KeyboardEvent = e.unchecked_ref();
                    if k.key() == "Enter" && !k.is_composing() {
                        let _ = i3.blur();
                    }
                });
                ol.append_child(&li).unwrap();
            }
        }
    }

    async fn reload_sw() {
        let sw: Stopwatch = chrome::get_or(SW_KEY, Stopwatch::default()).await;
        SW.with(|s| *s.borrow_mut() = sw);
        render_sw();
        ensure_ticking();
    }

    fn act_sw(f: impl FnOnce(&mut Stopwatch, f64) + 'static) {
        spawn(async move {
            let mut sw = SW.with(|s| s.borrow().clone());
            f(&mut sw, chrome::now());
            chrome::set(&[(SW_KEY, chrome::to_js(&sw))]).await;
            reload_sw().await;
        });
    }

    fn tick() {
        render();
        render_sw();
    }

    fn render() {
        let now = chrome::now();
        let t = CURRENT.with(|c| c.borrow().clone());
        let running = t.is_some();
        hide("tm-presets-wrap", running);
        hide("tm-run", !running);
        // 標題列：計時中顯示剩餘時間
        match &t {
            Some(t) => {
                let left = t.left(now);
                let txt = mmss(left);
                el("todo-timer").set_text_content(Some(&format!("{} {txt}", if t.paused() { "⏸" } else { "⏱" })));
                hide("todo-timer", false);
                el("tm-left").set_text_content(Some(&if left <= 0.0 { "時間到！".to_string() } else { txt }));
                el("tm-label").set_text_content(Some(&format!("{}計時{}", duration_label(t.seconds), if t.paused() { "（已暫停）" } else { "" })));
                let _ = el("tm-bar").style().set_property("width", &format!("{:.1}%", t.progress(now) * 100.0));
                el("tm-pause").set_text_content(Some(if t.paused() { "▶ 繼續" } else { "⏸ 暫停" }));
            }
            None => hide("todo-timer", true),
        }
    }

    /// 有東西在跑才更新畫面：碼表每 0.05 秒、倒數每 0.25 秒
    fn ensure_ticking() {
        let cd = CURRENT.with(|c| c.borrow().as_ref().is_some_and(|t| !t.paused()));
        let sw = SW.with(|s| s.borrow().running());
        let want = if sw { 50 } else if cd { 250 } else { 0 };
        let (id, rate) = TICKING.with(|t| t.get());
        if want == rate {
            return;
        }
        if id != 0 {
            web_sys::window().unwrap().clear_interval_with_handle(id);
        }
        let mut new_id = 0;
        if want > 0 {
            let cb = Closure::<dyn FnMut()>::new(tick);
            new_id = web_sys::window()
                .unwrap()
                .set_interval_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), want)
                .unwrap_or(0);
            cb.forget();
        }
        TICKING.with(|t| t.set((new_id, want)));
    }

    async fn reload() {
        let t: Option<Timer> = chrome::get(KEY).await;
        CURRENT.with(|c| *c.borrow_mut() = t);
        render();
        ensure_ticking();
    }

    fn act(f: impl FnOnce(&mut Timer, f64) + 'static) {
        spawn(async move {
            let Some(mut t) = CURRENT.with(|c| c.borrow().clone()) else { return };
            f(&mut t, chrome::now());
            save(&t).await;
            reload().await;
        });
    }

    pub fn start() {
        // 點日期：展開／收起計時器
        let toggle = || {
            let open = el("tm-card").hidden();
            hide("tm-card", !open);
            let _ = el("todo-date").class_list().toggle_with_force("on", open);
        };
        on_click("todo-date", toggle);
        on_click("todo-timer", move || {
            show_tab(false);
            if el("tm-card").hidden() {
                toggle();
            }
        });
        on_click("todo-sw", move || {
            show_tab(true);
            if el("tm-card").hidden() {
                toggle();
            }
        });
        on_click("tm-tab-cd", || show_tab(false));
        on_click("tm-tab-sw", || show_tab(true));
        on_click("sw-toggle", || {
            act_sw(|sw, now| if sw.running() { sw.pause(now) } else { sw.start(now) });
        });
        on_click("sw-export", || {
            let sw = SW.with(|s| s.borrow().clone());
            let d = js_sys::Date::new_0();
            let name = format!(
                "碼表計次-{:04}{:02}{:02}-{:02}{:02}.csv",
                d.get_full_year(),
                d.get_month() + 1,
                d.get_date(),
                d.get_hours(),
                d.get_minutes()
            );
            crate::ui::download_text(&name, &sw_csv(&sw, chrome::now()), "text/csv;charset=utf-8");
        });
        on_click("sw-lap", || {
            act_sw(|sw, now| {
                if sw.running() {
                    sw.lap(now)
                } else {
                    *sw = Stopwatch::default()
                }
            });
        });
        for m in PRESETS {
            on_click(&format!("tm-{m}"), move || {
                spawn(async move {
                    start_timer(m * 60).await;
                    reload().await;
                })
            });
        }
        // 自訂時間
        let start_custom = || {
            let inp: web_sys::HtmlInputElement = el("tm-custom").unchecked_into();
            match parse_duration(&inp.value()) {
                Some(secs) => {
                    el("tm-msg").set_text_content(Some(""));
                    inp.set_value("");
                    spawn(async move {
                        start_timer(secs).await;
                        reload().await;
                    });
                }
                None => {
                    el("tm-msg").set_text_content(Some("請輸入 1 秒到 24 小時，例如：15、2.5、1:30、90秒"));
                    let _ = inp.focus();
                }
            }
        };
        on_click("tm-go", start_custom);
        crate::ui::listen(&el("tm-custom"), "keydown", move |e| {
            let k: &web_sys::KeyboardEvent = e.unchecked_ref();
            if k.key() == "Enter" && !k.is_composing() {
                e.prevent_default();
                start_custom();
            }
        });
        on_click("tm-pause", || {
            act(|t, now| if t.paused() { t.resume(now) } else { t.pause(now) });
        });
        on_click("tm-cancel", || {
            spawn(async {
                cancel().await;
                reload().await;
            })
        });
        // 其他分頁或提醒視窗改了計時器
        let cb = Closure::<dyn FnMut(JsValue, JsValue)>::new(|changes: JsValue, _area: JsValue| {
            if js_sys::Reflect::has(&changes, &KEY.into()).unwrap_or(false) {
                spawn(reload());
            }
            if js_sys::Reflect::has(&changes, &SW_KEY.into()).unwrap_or(false) {
                spawn(reload_sw());
            }
        });
        chrome::on_storage_changed(&cb);
        cb.forget();
        spawn(async {
            reload().await;
            reload_sw().await;
            // 只有碼表在跑時，預設顯示碼表分頁
            let sw_on = SW.with(|s| s.borrow().running());
            let cd_on = CURRENT.with(|c| c.borrow().is_some());
            show_tab(sw_on && !cd_on);
        });
    }
}

pub use view::start;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown() {
        let mut t = Timer::start(300, 1_000.0);
        assert_eq!(mmss(t.left(1_000.0)), "05:00");
        assert_eq!(mmss(t.left(2_500.0)), "04:59");
        assert_eq!(mmss(t.left(1_000.0 + 300_000.0)), "00:00");
        assert_eq!(t.left(10_000_000.0), 0.0);
        // 暫停 → 時間不動；繼續 → 從剩下的接著倒數
        t.pause(61_000.0);
        assert!(t.paused());
        assert_eq!(t.left(200_000.0), 240_000.0);
        t.resume(500_000.0);
        assert_eq!(t.end, 740_000.0);
        assert_eq!(mmss(t.left(500_000.0)), "04:00");
        assert!((t.progress(500_000.0) - 0.2).abs() < 1e-9);
        assert_eq!(mmss(3_725_000.0), "1:02:05");
    }

    #[test]
    fn stopwatch() {
        let mut sw = Stopwatch::default();
        assert_eq!(sw.elapsed(5.0), 0.0);
        sw.start(1_000.0);
        assert!(sw.running());
        assert_eq!(sw_fmt(sw.elapsed(13_340.0)), "00:12.34");
        sw.lap(6_000.0);
        sw.lap(16_000.0);
        sw.pause(21_000.0);
        assert!(!sw.running());
        assert_eq!(sw.elapsed(99_999.0), 20_000.0);
        sw.lap(30_000.0); // 暫停時不能計次
        assert_eq!(sw.lap_rows(), vec![(2, 10_000.0, 15_000.0), (1, 5_000.0, 5_000.0)]);
        sw.start(50_000.0);
        assert_eq!(sw.elapsed(51_000.0), 21_000.0);
        assert_eq!(sw_fmt(3_723_450.0), "1:02:03.45");
        sw.pause(52_000.0);
        sw.set_note(2, "第二圈, 有點喘");
        sw.set_note(9, "不存在的計次");
        assert_eq!((sw.note(1), sw.note(2), sw.note(9)), ("", "第二圈, 有點喘", ""));
        assert_eq!(
            sw_csv(&sw, 0.0),
            "\u{feff}計次,筆記,分段時間,累計時間\r\n1,,00:05.00,00:05.00\r\n2,\"第二圈, 有點喘\",00:10.00,00:15.00\r\n總時間,,,00:22.00\r\n"
        );
        sw.set_note(2, "");
        assert!(sw.notes.is_empty());

        // 移除計次：那一段併到下一次，筆記跟著走
        let mut sw = Stopwatch { laps: vec![5.0, 15.0, 30.0], notes: vec!["a".into(), "b".into(), "c".into()], ..Default::default() };
        sw.remove_lap(2);
        assert_eq!(sw.laps, vec![5.0, 30.0]);
        assert_eq!(sw.notes, vec!["a".to_string(), "c".to_string()]);
        assert_eq!(sw.lap_rows(), vec![(2, 25.0, 30.0), (1, 5.0, 5.0)]);
        sw.remove_lap(9);
        sw.remove_lap(0);
        assert_eq!(sw.laps.len(), 2);
        sw.remove_lap(2);
        sw.remove_lap(1);
        assert!(sw.laps.is_empty() && sw.notes.is_empty());
    }

    #[test]
    fn custom_input() {
        assert_eq!(parse_duration("15"), Some(900));
        assert_eq!(parse_duration(" 2.5 "), Some(150));
        assert_eq!(parse_duration("1:30"), Some(90));
        assert_eq!(parse_duration("1：30"), Some(90));
        assert_eq!(parse_duration("1:05:00"), Some(3900));
        assert_eq!(parse_duration("90秒"), Some(90));
        assert_eq!(parse_duration("20分"), Some(1200));
        assert_eq!(parse_duration("20分鐘"), Some(1200));
        assert_eq!(parse_duration("1小時"), Some(3600));
        assert_eq!(parse_duration("0"), None);
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration("-5"), None);
        assert_eq!(parse_duration("25:00:01"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(duration_label(300), "5 分鐘");
        assert_eq!(duration_label(90), "1 分 30 秒");
        assert_eq!(duration_label(45), "45 秒");
        assert_eq!(duration_label(5400), "1 小時 30 分鐘");
        assert_eq!(duration_label(3600), "1 小時");
    }
}
