pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
