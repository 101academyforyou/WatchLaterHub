//! WatchLaterHub 電腦小幫手（Chrome Native Messaging host）
//!
//! Chrome 擴充功能不能直接開啟電腦上的程式，這個小程式由 Chrome 啟動，
//! 透過 stdin/stdout（4 位元組長度 + JSON）接收指令：
//!
//! - `{"action":"ping"}` → `{"ok":true,"version":1,"os":"macos"}`
//! - `{"action":"list"}` → `{"ok":true,"apps":[{"name":"Safari","path":"/Applications/Safari.app"}…]}`
//! - `{"action":"open","path":"…"}` → 開啟那個軟體
//! - `{"action":"icon","path":"…"}` → `{"ok":true,"icon":"data:image/png;base64,…"}`（軟體的圖示）
//!
//! 安全：只會開啟「已安裝軟體清單」裡的項目，不執行任何其他指令。

use serde::Serialize;
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Clone, Debug, PartialEq)]
struct App {
    name: String,
    path: String,
}

// ---------- 訊息格式 ----------

fn read_message(r: &mut impl Read) -> io::Result<Option<Value>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let n = u32::from_le_bytes(len) as usize;
    if n > 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message too large"));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    serde_json::from_slice(&buf).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_message(w: &mut impl Write, v: &Value) -> io::Result<()> {
    let s = serde_json::to_vec(v)?;
    w.write_all(&(s.len() as u32).to_le_bytes())?;
    w.write_all(&s)?;
    w.flush()
}

// ---------- 已安裝軟體 ----------

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn list_apps() -> Vec<App> {
    let mut dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/Applications/Utilities"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
        home().join("Applications"),
    ];
    // /Applications 底下一層資料夾裡的 .app（例如 Microsoft Office 套件）
    if let Ok(rd) = std::fs::read_dir("/Applications") {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && p.extension().is_none() {
                dirs.push(p);
            }
        }
    }
    let mut apps = vec![];
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("app") {
                let name = p.file_stem().and_then(|x| x.to_str()).unwrap_or_default().to_string();
                apps.push(App { name, path: p.to_string_lossy().into_owned() });
            }
        }
    }
    finish(apps)
}

#[cfg(not(target_os = "macos"))]
fn list_apps() -> Vec<App> {
    // Linux：.desktop 檔
    let mut apps = vec![];
    for d in [PathBuf::from("/usr/share/applications"), PathBuf::from("/usr/local/share/applications"), home().join(".local/share/applications")] {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            if desktop_field(&text, "NoDisplay").as_deref() == Some("true") || desktop_field(&text, "Exec").is_none() {
                continue;
            }
            if let Some(name) = desktop_field(&text, "Name") {
                apps.push(App { name, path: p.to_string_lossy().into_owned() });
            }
        }
    }
    finish(apps)
}

/// .desktop 檔裡 [Desktop Entry] 區段的欄位
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn desktop_field(text: &str, key: &str) -> Option<String> {
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_entry {
            if let Some(v) = line.strip_prefix(key).and_then(|r| r.strip_prefix('=')) {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

/// 依名稱排序、去重
fn finish(mut apps: Vec<App>) -> Vec<App> {
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.path.cmp(&b.path)));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

// ---------- 開啟 ----------

#[cfg(target_os = "macos")]
fn launch(path: &Path) -> io::Result<()> {
    Command::new("/usr/bin/open").arg(path).spawn().map(|_| ())
}

#[cfg(not(target_os = "macos"))]
fn launch(path: &Path) -> io::Result<()> {
    let text = std::fs::read_to_string(path)?;
    let exec = desktop_field(&text, "Exec").ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no Exec"))?;
    let args = exec_args(&exec);
    let (prog, rest) = args.split_first().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty Exec"))?;
    Command::new(prog).args(rest).spawn().map(|_| ())
}

/// Exec 欄位拆成參數，去掉 %U %f 這類欄位代碼
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn exec_args(exec: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    for c in exec.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter().filter(|a| !(a.len() == 2 && a.starts_with('%'))).collect()
}

// ---------- 圖示 ----------

/// 圖示邊長（新分頁顯示 40px，高解析度螢幕要 2 倍）
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const ICON_PX: u32 = 96;

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(bytes))
}

/// 暫存檔（每次不同名稱，用完刪掉）
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn temp_png() -> PathBuf {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("watchlaterhub-icon-{}-{t}.png", std::process::id()))
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn take_png(out: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(out).ok();
    let _ = std::fs::remove_file(out);
    bytes.filter(|b| b.starts_with(b"\x89PNG"))
}

/// macOS：先把軟體內附的 .icns 用 sips 轉成 PNG；沒有 .icns（圖示放在 Assets.car）再請系統
/// （NSWorkspace，和 Finder 看到的一樣）輸出圖示，最後都縮成 ICON_PX
#[cfg(target_os = "macos")]
fn app_icon(path: &Path) -> Option<String> {
    let px = ICON_PX.to_string();
    let quiet = |c: &mut Command| c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success());
    if let Some(icns) = icns_file(path) {
        let out = temp_png();
        if quiet(Command::new("/usr/bin/sips").args(["-s", "format", "png", "-Z", &px]).arg(&icns).arg("--out").arg(&out)) {
            if let Some(b) = take_png(&out) {
                return Some(data_url("image/png", &b));
            }
        }
        let _ = std::fs::remove_file(&out);
    }
    // 取系統圖示裡最接近 128px 的那一張（不用自己畫，避免得到透明的空白圖）
    const JXA: &str = r#"
ObjC.import('AppKit');
function run(argv) {
  var img = $.NSWorkspace.sharedWorkspace.iconForFile(argv[0]);
  var reps = $.NSBitmapImageRep.imageRepsWithData(img.TIFFRepresentation);
  var best = null, bw = 0;
  for (var i = 0; i < reps.count; i++) {
    var r = reps.objectAtIndex(i), w = r.pixelsWide;
    if (best === null || (bw < 128 ? w > bw : (w >= 128 && w < bw))) { best = r; bw = w; }
  }
  if (best === null) return;
  best.representationUsingTypeProperties($.NSBitmapImageFileTypePNG, $()).writeToFileAtomically(argv[1], true);
}
"#;
    let out = temp_png();
    if quiet(Command::new("/usr/bin/osascript").args(["-l", "JavaScript", "-e", JXA]).arg(path).arg(&out)) {
        let _ = quiet(Command::new("/usr/bin/sips").args(["-Z", &px]).arg(&out).arg("--out").arg(&out));
        if let Some(b) = take_png(&out) {
            return Some(data_url("image/png", &b));
        }
    }
    let _ = std::fs::remove_file(&out);
    None
}

#[cfg(target_os = "macos")]
fn icns_file(app: &Path) -> Option<PathBuf> {
    let res = app.join("Contents/Resources");
    let named = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
        .arg(app.join("Contents/Info.plist"))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(n) = named {
        let p = res.join(if n.ends_with(".icns") { n } else { format!("{n}.icns") });
        if p.is_file() {
            return Some(p);
        }
    }
    let mut all: Vec<PathBuf> = std::fs::read_dir(&res).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|x| x.to_str()) == Some("icns")).collect();
    all.sort_by_key(|p| !p.file_stem().and_then(|x| x.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("AppIcon")));
    all.into_iter().next()
}

/// Linux：.desktop 的 Icon 欄位（絕對路徑，或在圖示主題裡找同名檔案）
#[cfg(not(target_os = "macos"))]
fn app_icon(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let icon = desktop_field(&text, "Icon")?;
    let file = if icon.starts_with('/') { Some(PathBuf::from(&icon)).filter(|p| p.is_file()) } else { find_theme_icon(&icon) }?;
    let mime = match file.extension().and_then(|x| x.to_str()) {
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        _ => return None,
    };
    let bytes = std::fs::read(&file).ok()?;
    if bytes.len() > 512 * 1024 {
        return None;
    }
    Some(data_url(mime, &bytes))
}

#[cfg(not(target_os = "macos"))]
fn find_theme_icon(name: &str) -> Option<PathBuf> {
    let mut bases = vec![home().join(".local/share/icons"), home().join(".icons")];
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    bases.extend(data_dirs.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("icons")));
    // 優先接近 96px 的 PNG，再來是向量圖
    let sizes = ["96x96", "128x128", "64x64", "256x256", "72x72", "48x48", "512x512", "scalable"];
    for base in &bases {
        let mut themes = vec![PathBuf::from("hicolor")];
        if let Ok(rd) = std::fs::read_dir(base) {
            let mut others: Vec<PathBuf> = rd.flatten().map(|e| PathBuf::from(e.file_name())).filter(|t| t.as_os_str() != "hicolor").collect();
            others.sort();
            themes.extend(others);
        }
        for theme in &themes {
            for size in sizes {
                for ext in ["png", "svg"] {
                    let p = base.join(theme).join(size).join("apps").join(format!("{name}.{ext}"));
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
    }
    ["png", "svg"].iter().map(|ext| PathBuf::from(format!("/usr/share/pixmaps/{name}.{ext}"))).find(|p| p.is_file())
}

// ---------- 指令 ----------

fn handle(msg: &Value) -> Value {
    match msg["action"].as_str() {
        Some("ping") => json!({ "ok": true, "version": 1, "os": std::env::consts::OS }),
        Some("list") => json!({ "ok": true, "apps": list_apps() }),
        Some("open") => {
            let Some(path) = msg["path"].as_str() else { return json!({ "ok": false, "error": "缺少 path" }) };
            // 只開啟清單裡的軟體
            if !list_apps().iter().any(|a| a.path == path) {
                return json!({ "ok": false, "error": "找不到這個軟體（可能已移除）" });
            }
            match launch(Path::new(path)) {
                Ok(()) => json!({ "ok": true }),
                Err(e) => json!({ "ok": false, "error": e.to_string() }),
            }
        }
        Some("icon") => {
            let Some(path) = msg["path"].as_str() else { return json!({ "ok": false, "error": "缺少 path" }) };
            // 只讀清單裡軟體的圖示
            if !list_apps().iter().any(|a| a.path == path) {
                return json!({ "ok": false, "error": "找不到這個軟體（可能已移除）" });
            }
            match app_icon(Path::new(path)) {
                Some(icon) => json!({ "ok": true, "icon": icon }),
                None => json!({ "ok": false, "error": "沒有圖示" }),
            }
        }
        _ => json!({ "ok": false, "error": "未知的指令" }),
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let (mut r, mut w) = (stdin.lock(), stdout.lock());
    // sendNativeMessage 每次只送一則；connectNative 可能送很多則
    while let Ok(Some(msg)) = read_message(&mut r) {
        if write_message(&mut w, &handle(&msg)).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_round_trip() {
        let mut buf = vec![];
        write_message(&mut buf, &json!({"action": "ping"})).unwrap();
        assert_eq!(&buf[..4], &(17u32).to_le_bytes());
        let v = read_message(&mut &buf[..]).unwrap().unwrap();
        assert_eq!(v["action"], "ping");
        assert!(read_message(&mut &b""[..]).unwrap().is_none());
    }

    #[test]
    fn commands() {
        assert_eq!(handle(&json!({"action": "ping"}))["ok"], true);
        assert_eq!(handle(&json!({"action": "nope"}))["ok"], false);
        // 不在清單裡的路徑不會執行
        let r = handle(&json!({"action": "open", "path": "/bin/rm"}));
        assert_eq!(r["ok"], false);
    }

    #[test]
    fn base64_encoding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        let r = handle(&json!({"action": "icon", "path": "/etc/passwd"}));
        assert_eq!(r["ok"], false);
    }

    #[test]
    fn desktop_parsing() {
        let t = "[Desktop Entry]\nName=Text Editor\nExec=gedit %U --new-window\n[Desktop Action x]\nName=Other\n";
        assert_eq!(desktop_field(t, "Name").as_deref(), Some("Text Editor"));
        assert_eq!(exec_args(&desktop_field(t, "Exec").unwrap()), ["gedit", "--new-window"]);
        assert_eq!(exec_args(r#""/opt/My App/app" --flag %f"#), ["/opt/My App/app", "--flag"]);
    }
}
