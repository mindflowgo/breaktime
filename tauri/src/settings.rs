use serde_json::Value;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn settings_file_path(app: &AppHandle) -> PathBuf {
    if let Ok(app_dir) = app.path().app_data_dir() {
        return app_dir.join(".settings.json");
    }
    PathBuf::from(".settings.json")
}

pub fn get_settings_raw(app: &AppHandle) -> Value {
    let path = settings_file_path(app);
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str(&content) {
                return json;
            }
        }
    }
    serde_json::json!({})
}

pub fn get_setting_value(app: &AppHandle, key: &str) -> Option<Value> {
    let raw = get_settings_raw(app);
    raw.get(key).cloned()
}

/// Atomic write: a crash mid-write must never truncate the settings file,
/// so write to a sibling temp file and rename over the target.
pub fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_setting_value(app: &AppHandle, key: String, value: Value) -> Result<(), String> {
    let path = settings_file_path(app);

    let mut raw = get_settings_raw(app);
    if let Some(obj) = raw.as_object_mut() {
        obj.insert(key, value);
    } else {
        raw = serde_json::json!({ key: value });
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }

    let content = serde_json::to_string_pretty(&raw).map_err(|e| e.to_string())?;
    atomic_write(&path, &content)?;
    Ok(())
}

#[tauri::command]
pub fn get_setting(app: tauri::AppHandle, key: Option<String>) -> Result<serde_json::Value, String> {
    if let Some(k) = key {
        Ok(get_setting_value(&app, &k).unwrap_or(serde_json::Value::Null))
    } else {
        Ok(get_settings_raw(&app))
    }
}

#[tauri::command]
pub fn set_setting(app: tauri::AppHandle, key: String, value: serde_json::Value) -> Result<(), String> {
    set_setting_value(&app, key, value)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_settings_json_roundtrip() {
        let mut raw = serde_json::json!({
            "settings": {
                "eyeEnabled": true,
                "eyeIntervalMin": 20,
                "eyeDurationSec": 20,
                "stretchEnabled": true,
                "stretchIntervalMin": 45,
                "stretchDurationSec": 60,
                "soundPreset": "gentle-chime",
                "soundVolume": 0.6,
                "soundFadeIn": true,
                "idleThresholdSec": 300,
                "awayResetMin": 10
            },
            "stats": {
                "totalCompleted": 5,
                "totalSkipped": 1,
                "eyeCompleted": 3,
                "eyeSkipped": 1,
                "stretchCompleted": 2,
                "stretchSkipped": 0,
                "events": []
            }
        });

        assert_eq!(raw["settings"]["eyeIntervalMin"], 20);
        assert_eq!(raw["stats"]["totalCompleted"], 5);

        if let Some(obj) = raw.as_object_mut() {
            obj.insert("custom_key".to_string(), serde_json::json!("test_val"));
        }
        assert_eq!(raw["custom_key"], "test_val");
    }
}

