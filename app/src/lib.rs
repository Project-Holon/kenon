//! The Tauri shell. It holds the settings and exposes the core to the front
//! end as commands. It parses no game formats itself.

mod settings;

use std::path::PathBuf;
use std::sync::Mutex;

use kenon_core::data_root::{self, DataRootCheck};
use serde::Serialize;
use tauri::{Manager, State};

use settings::Settings;

struct AppState {
    settings: Mutex<Settings>,
    settings_path: PathBuf,
}

/// What the settings screen shows about the data folder.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DataFolderStatus {
    /// The folder saved in the settings, if any.
    folder: Option<PathBuf>,
    /// The check of that folder, if one is set.
    check: Option<DataRootCheck>,
    /// The eidos default extraction folder, offered as a suggestion.
    suggestion: Option<PathBuf>,
}

fn status(folder: Option<PathBuf>) -> DataFolderStatus {
    DataFolderStatus {
        check: folder.as_deref().map(data_root::check),
        folder,
        suggestion: data_root::eidos_default().filter(|dir| dir.is_dir()),
    }
}

#[tauri::command]
fn data_folder_status(state: State<AppState>) -> DataFolderStatus {
    let folder = state.settings.lock().unwrap().data_folder.clone();
    status(folder)
}

/// Saves the data folder (or clears it with `None`) and returns its check.
/// A folder that fails the check is still saved, so the user sees why.
#[tauri::command]
fn set_data_folder(
    state: State<AppState>,
    folder: Option<PathBuf>,
) -> Result<DataFolderStatus, String> {
    let mut settings = state.settings.lock().unwrap();
    settings.data_folder = folder.clone();
    settings
        .save(&state.settings_path)
        .map_err(|err| format!("could not save the settings: {err}"))?;
    Ok(status(folder))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&settings_path);
            app.manage(AppState {
                settings: Mutex::new(settings),
                settings_path,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            data_folder_status,
            set_data_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running kenon");
}
