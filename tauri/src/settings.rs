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

/// Atomic write: a crash mid-write must never truncate the target file, so
/// write to a sibling temp file and rename over the target. The temp name is
/// unique per call (pid + counter) so concurrent writers on different threads
/// cannot interleave on a shared tmp path.
pub fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "state".to_string());
    let tmp = path.with_file_name(format!(
        ".{}.{}.{}.tmp",
        file_name,
        std::process::id(),
        seq
    ));
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
    use super::atomic_write;

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_tmp() {
        let dir = std::env::temp_dir().join(format!("breaktime-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");

        atomic_write(&path, "{\"a\":1}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":1}");

        atomic_write(&path, "{\"a\":2}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}");

        // No temp files left behind
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "leftover tmp files: {leftovers:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_creates_missing_parent_dirs_target_only() {
        // atomic_write itself does not create parents (callers do); verify the
        // error is surfaced instead of panicking.
        let path = std::path::PathBuf::from("/nonexistent-breaktime-dir/x.json");
        assert!(atomic_write(&path, "{}").is_err());
    }
}

