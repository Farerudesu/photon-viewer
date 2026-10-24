use std::path::PathBuf;

pub fn get_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("PhotonViewer").join("config.json");
    }
    PathBuf::from("photon_config.json")
}

pub fn load_confirm_delete() -> bool {
    let p = get_config_path();
    if let Ok(content) = std::fs::read_to_string(p) {
        if content.contains("\"confirm_delete\": false") {
            return false;
        }
    }
    true
}

pub fn save_confirm_delete(confirm: bool) {
    let p = get_config_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let content = format!("{{\n  \"confirm_delete\": {}\n}}\n", confirm);
    let _ = std::fs::write(p, content);
}
