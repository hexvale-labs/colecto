// `press` is implemented in Kotlin and Swift; the Rust side only registers the native plugin.
const COMMANDS: &[&str] = &["press"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
