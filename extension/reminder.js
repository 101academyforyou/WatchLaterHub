// 只負責載入 Rust 編譯出來的 WebAssembly；邏輯在 src/reminder_page.rs
import init, { start_reminder } from "./pkg/watchlaterhub.js";

await init();
start_reminder();
