//! The Tauri shell. It holds the settings and exposes the core to the front
//! end as commands. It parses no game formats itself.

mod protocol;
mod settings;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use kenon_core::bridge::Bridge;
use kenon_core::data_root::{self, DataRootCheck};
use serde::Serialize;
use tauri::{Manager, State};

use settings::Settings;

struct AppState {
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    /// The asset bridge over the data folder, when the folder holds game data.
    bridge: RwLock<Option<Arc<Bridge>>>,
}

impl AppState {
    fn bridge(&self) -> Option<Arc<Bridge>> {
        self.bridge.read().unwrap().clone()
    }
}

fn bridge_for(folder: Option<&Path>) -> Option<Arc<Bridge>> {
    match data_root::check(folder?) {
        DataRootCheck::Ready(root) => Some(Arc::new(Bridge::new(root))),
        _ => None,
    }
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
    *state.bridge.write().unwrap() = bridge_for(folder.as_deref());
    Ok(status(folder))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&settings_path);
            let bridge = bridge_for(settings.data_folder.as_deref());
            app.manage(AppState {
                settings: Mutex::new(settings),
                settings_path,
                bridge: RwLock::new(bridge),
            });
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("kenon", |ctx, request, responder| {
            let bridge = ctx.app_handle().state::<AppState>().bridge();
            let uri = request.uri().clone();
            // Decoding can take a while; keep it off the event loop.
            std::thread::spawn(move || {
                responder.respond(protocol::respond(bridge, uri.path(), uri.query()));
            });
        })
        .invoke_handler(tauri::generate_handler![
            data_folder_status,
            set_data_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running kenon");
}
