// 只負責載入 Rust 編譯出來的 WebAssembly；所有邏輯都在 src/*.rs
import init, { start_newtab } from "./pkg/watchlaterhub.js";

await init();
start_newtab();
