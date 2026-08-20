pub fn packaged_invoke_url() -> tauri::Url {
    let origin = if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    origin.parse().expect("fixed packaged origin must be valid")
}
