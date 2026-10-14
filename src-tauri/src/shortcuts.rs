use crate::{
    library::{atomic_write, err, Result},
    AppState,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
    sync::Arc,
};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut};

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub edge: String,
    pub display: Option<String>,
    pub shortcuts: BTreeMap<String, String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            edge: "right".into(),
            display: None,
            shortcuts: [
                ("region", "Ctrl+Alt+1"),
                ("window", "Ctrl+Alt+2"),
                ("screen", "Ctrl+Alt+3"),
                ("toggle", "Ctrl+Alt+Space"),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect(),
        }
    }
}
pub struct StoredSettings {
    pub value: Settings,
    pub writable: bool,
    pub warning: Option<String>,
}
pub fn validate(settings: &Settings) -> Result<BTreeMap<String, Shortcut>> {
    if !["left", "right"].contains(&settings.edge.as_str()) {
        return Err("Choose the left or right edge.".into());
    }
    let expected = ["region", "window", "screen", "toggle"];
    if settings.shortcuts.len() != 4
        || expected
            .iter()
            .any(|k| !settings.shortcuts.contains_key(*k))
    {
        return Err("Four shortcut bindings are required.".into());
    }
    let mut ids = HashSet::new();
    settings
        .shortcuts
        .iter()
        .map(|(action, binding)| {
            let shortcut: Shortcut = binding
                .parse()
                .map_err(|e| format!("Invalid {action} shortcut: {e}"))?;
            let function_key = shortcut
                .key
                .to_string()
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=24).contains(&n));
            if !function_key
                && !shortcut
                    .mods
                    .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
            {
                return Err("Global shortcuts need Ctrl, Alt or Windows as a modifier.".into());
            }
            if !ids.insert(shortcut.id()) {
                return Err("Each action needs a different shortcut.".into());
            }
            Ok((action.clone(), shortcut))
        })
        .collect()
}
pub fn load(root: &Path) -> Result<StoredSettings> {
    match fs::read(root.join("settings.json")) {
        Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes).ok().filter(|s| validate(s).is_ok()) {
            Some(value) => Ok(StoredSettings { value, writable: true, warning: None }),
            None => Ok(StoredSettings { value: Settings::default(), writable: false, warning: Some("Settings are damaged and have been preserved. Defaults are active; recover settings.json before saving changes.".into()) }),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StoredSettings { value: Settings::default(), writable: true, warning: None }),
        Err(e) => Err(err(e)),
    }
}
pub fn register_initial(app: &AppHandle, settings: &Settings) -> Option<String> {
    let parsed = match validate(settings) {
        Ok(p) => p,
        Err(e) => return Some(e),
    };
    let failures: Vec<_> = parsed
        .into_iter()
        .filter_map(|(k, v)| {
            app.global_shortcut()
                .register(v)
                .err()
                .map(|e| format!("{k}: {e}"))
        })
        .collect();
    if failures.is_empty() {
        None
    } else {
        Some(format!(
            "Some shortcuts are unavailable. Change them in Settings. {}",
            failures.join("; ")
        ))
    }
}
pub fn update(app: &AppHandle, next: Settings) -> Result<Settings> {
    let new = validate(&next)?;
    if let Some(id) = &next.display {
        if !crate::shelf_window::displays()?.iter().any(|d| &d.id == id) {
            return Err("That display is disconnected. Choose an available display.".into());
        }
    }
    let state = app.state::<Arc<AppState>>();
    let mut settings = state.settings.lock().unwrap();
    if !settings.writable {
        return Err(settings
            .warning
            .clone()
            .unwrap_or("Settings need recovery.".into()));
    }
    let old = validate(&settings.value)?;
    let mut added = Vec::new();
    for shortcut in new.values() {
        if app.global_shortcut().is_registered(*shortcut) {
            continue;
        }
        if let Err(e) = app.global_shortcut().register(*shortcut) {
            eprintln!("Shortcut registration failed: {e}");
            for s in added {
                let _ = app.global_shortcut().unregister(s);
            }
            return Err("That shortcut could not be registered. It may be in use by another app. Choose another combination; previous shortcuts are unchanged.".into());
        }
        added.push(*shortcut);
    }
    let root = state.library.lock().unwrap().root.clone();
    if let Err(e) = atomic_write(
        &root.join("settings.json"),
        &serde_json::to_vec_pretty(&next).map_err(err)?,
    ) {
        for s in added {
            let _ = app.global_shortcut().unregister(s);
        }
        return Err(e);
    }
    settings.value = next.clone();
    for shortcut in old
        .values()
        .filter(|s| !new.values().any(|n| n.id() == s.id()))
    {
        if let Err(e) = app.global_shortcut().unregister(*shortcut) {
            state.shelf.lock().unwrap().view.error = Some(format!("Settings saved, but a previous shortcut could not be released: {e}. Restart Captrain to release it."));
        }
    }
    drop(settings);
    crate::shelf_window::apply(app, false);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_duplicates_modifiers_and_actions() {
        let mut s = Settings::default();
        assert!(validate(&s).is_ok());
        s.shortcuts.insert("region".into(), "A".into());
        assert!(validate(&s).is_err());
        s.shortcuts.insert("region".into(), "Ctrl+Alt+2".into());
        assert!(validate(&s).is_err());
        s = Settings::default();
        s.shortcuts.remove("screen");
        assert!(validate(&s).is_err());
    }
}
