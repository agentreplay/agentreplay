use tauri::{AppHandle, Manager, WebviewWindowBuilder, WebviewUrl};

pub fn open_memory_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("memory") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let _window = WebviewWindowBuilder::new(
        app,
        "memory",
        WebviewUrl::App("index.html#/memory".into()),
    )
    .title("Agentreplay Memory")
    .inner_size(1200.0, 800.0)
    .min_inner_size(800.0, 600.0)
    .resizable(true)
    .visible(true)
    .build();
    
    // Center the window on creation
    if let Ok(window) = _window {
       let _ = window.center();
    }
}
