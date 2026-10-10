use base64::Engine;
use tauri::Manager;

const SOUND_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "m4a", "aac", "flac"];
const CUSTOM_SOUND_BASE: &str = "custom-sound";

/// Copy a user-picked audio file into the app data dir as
/// `custom-sound.<ext>` and return the stored file name. The webview cannot
/// read arbitrary paths itself, so the heavy lifting happens here.
#[tauri::command]
pub fn import_custom_sound<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    source: String,
) -> Result<String, String> {
    let src = std::path::PathBuf::from(&source);
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !SOUND_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!(
            "Unsupported audio type \"{}\" — use {}",
            ext,
            SOUND_EXTENSIONS.join(", ")
        ));
    }
    if !src.is_file() {
        return Err("Selected file does not exist".to_string());
    }

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let file_name = format!("{}.{}", CUSTOM_SOUND_BASE, ext);
    let dest = dir.join(&file_name);
    std::fs::copy(&src, &dest).map_err(|e| format!("Could not copy file: {}", e))?;
    Ok(file_name)
}

/// The custom sound as a base64 data URL the webview <audio> can play.
/// Returns Null if no custom sound file exists.
#[tauri::command]
pub fn get_custom_sound_data<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    file_name: String,
) -> Result<serde_json::Value, String> {
    // Only serve files we handed out ourselves (no path traversal).
    let name = std::path::Path::new(&file_name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if !name.starts_with(CUSTOM_SOUND_BASE) {
        return Ok(serde_json::Value::Null);
    }

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let path = dir.join(&name);
    if !path.is_file() {
        return Ok(serde_json::Value::Null);
    }

    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mime = match name.rsplit('.').next().unwrap_or("") {
        "wav" => "wav",
        "ogg" => "ogg",
        "flac" => "flac",
        "aac" => "aac",
        "m4a" => "mp4",
        _ => "mpeg",
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(serde_json::Value::String(format!(
        "data:audio/{};base64,{}",
        mime, encoded
    )))
}

/// Remove any stale custom-sound.* variants except the current one.
#[tauri::command]
pub fn cleanup_custom_sounds<R: tauri::Runtime>(app: tauri::AppHandle<R>, keep: String) {
    if let Ok(dir) = app.path().app_data_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(CUSTOM_SOUND_BASE) && name != keep {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
}
