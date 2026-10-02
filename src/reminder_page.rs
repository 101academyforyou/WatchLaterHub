//! 提醒小視窗（reminder.html?id=…）：時間到時由背景跳出

use crate::chrome;
use crate::todo::{reminders, view::when_label};
use crate::ui::{el, hide, on_click, spawn, text};
use wasm_bindgen::JsCast;
use web_sys::{HtmlButtonElement, KeyboardEvent};

fn close() {
    let _ = web_sys::window().unwrap().close();
}

/// 輕輕「叮咚」兩聲（瀏覽器不允許時就安靜略過）
fn chime() {
    let Ok(ctx) = web_sys::AudioContext::new() else { return };
    let t0 = ctx.current_time();
    for (i, freq) in [880.0_f32, 660.0].iter().enumerate() {
        let (Ok(osc), Ok(gain)) = (ctx.create_oscillator(), ctx.create_gain()) else { return };
        let start = t0 + i as f64 * 0.28;
        osc.frequency().set_value(*freq);
        let g = gain.gain();
        g.set_value(0.0);
        let _ = g.linear_ramp_to_value_at_time(0.25, start + 0.02);
        let _ = g.exponential_ramp_to_value_at_time(0.001, start + 0.5);
        let _ = osc.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&ctx.destination());
        let _ = osc.start_with_when(start);
        let _ = osc.stop_with_when(start + 0.55);
    }
}

fn set_busy(b: bool) {
    for id in ["r-done", "r-later-5", "r-later-10", "r-later-30", "r-later-60", "r-ok", "r-open"] {
        el(id).unchecked_into::<HtmlButtonElement>().set_disabled(b);
    }
}

/// 計時器結束的提醒（reminder.html?timer=秒數）
fn start_timer_mode(seconds: &str) {
    let label = seconds.parse::<u32>().map(crate::timer::duration_label).unwrap_or_default();
    text("r-kind", "計時器");
    text("r-timer-text", &format!("{label}計時結束！"));
    crate::ui::doc().set_title(&format!("⏱ {label}計時結束"));
    hide("r-timer", false);
    hide("r-open", true);
    on_click("r-ok", close);
    for m in crate::timer::PRESETS {
        on_click(&format!("r-again-{m}"), move || {
            spawn(async move {
                crate::timer::background::start(m * 60).await;
                close();
            })
        });
    }
    crate::ui::listen(&web_sys::window().unwrap().into(), "keydown", |e| {
        if e.unchecked_ref::<KeyboardEvent>().key() == "Escape" {
            close();
        }
    });
    let _ = el("r-ok").focus();
    chime();
}

pub fn start() {
    let w = web_sys::window().unwrap();
    if let Some(m) = web_sys::UrlSearchParams::new_with_str(&w.location().search().unwrap_or_default())
        .ok()
        .and_then(|p| p.get("timer"))
    {
        return start_timer_mode(&m);
    }
    let id = web_sys::UrlSearchParams::new_with_str(&w.location().search().unwrap_or_default())
        .ok()
        .and_then(|p| p.get("id"))
        .unwrap_or_default();

    on_click("r-ok", close);
    {
        let id = id.clone();
        on_click("r-done", move || {
            let id = id.clone();
            set_busy(true);
            spawn(async move {
                reminders::done(&id).await;
                close();
            });
        });
    }
    for min in [5, 10, 30, 60] {
        let id = id.clone();
        on_click(&format!("r-later-{min}"), move || {
            let id = id.clone();
            set_busy(true);
            spawn(async move {
                reminders::snooze(&id, min as f64).await;
                close();
            });
        });
    }
    on_click("r-open", || {
        spawn(async {
            chrome::open_extension_page("newtab.html#todo").await;
            close();
        })
    });
    crate::ui::listen(&web_sys::window().unwrap().into(), "keydown", |e| {
        if e.unchecked_ref::<KeyboardEvent>().key() == "Escape" {
            close();
        }
    });

    spawn(async move {
        match reminders::get(&id).await {
            Some(t) if !t.done => {
                text("r-text", &t.text);
                if let Some(at) = t.remind_at {
                    text("r-when", &format!("提醒時間：{}", when_label(at)));
                }
                crate::ui::doc().set_title(&format!("⏰ {}", t.text));
                hide("r-main", false);
                let _ = el("r-done").focus();
                chime();
            }
            _ => {
                hide("r-gone", false);
                let _ = el("r-ok").focus();
            }
        }
    });
}
