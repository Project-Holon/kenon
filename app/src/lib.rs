//! The Tauri shell. It holds the settings and exposes the core to the front
//! end as commands. It parses no game formats itself.

mod protocol;
mod settings;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use kenon_core::bridge::Bridge;
use kenon_core::data_root;
use kenon_core::source::{self, GameCheck};
use serde::Serialize;
use tauri::{Manager, State};

use settings::Settings;

struct AppState {
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    /// The asset bridge over the chosen game, when it can be used.
    bridge: RwLock<Option<Arc<Bridge>>>,
}

impl AppState {
    fn bridge(&self) -> Option<Arc<Bridge>> {
        self.bridge.read().unwrap().clone()
    }
}

/// What the settings screen shows about the chosen game.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameStatus {
    /// The ROM or folder saved in the settings, if any.
    path: Option<PathBuf>,
    /// The check of it, if one is set.
    check: Option<GameCheck>,
    /// The eidos extraction folder, offered when it exists.
    suggestion: Option<PathBuf>,
}

/// Opens the chosen game: its check, and a bridge when it can be used.
fn open(path: Option<&Path>) -> (Option<GameCheck>, Option<Arc<Bridge>>) {
    let Some(path) = path else {
        return (None, None);
    };
    let (check, source) = source::open(path);
    (Some(check), source.map(|s| Arc::new(Bridge::new(s))))
}

fn status(path: Option<PathBuf>, check: Option<GameCheck>) -> GameStatus {
    GameStatus {
        path,
        check,
        suggestion: data_root::eidos_default().filter(|dir| dir.is_dir()),
    }
}

#[tauri::command]
fn game_status(state: State<AppState>) -> GameStatus {
    let path = state.settings.lock().unwrap().game.clone();
    let (check, _) = open(path.as_deref());
    status(path, check)
}

/// Saves the chosen ROM or folder (or clears it with `None`) and returns its
/// check. A choice that fails the check is still saved, so the user sees why.
#[tauri::command]
fn set_game(state: State<AppState>, path: Option<PathBuf>) -> Result<GameStatus, String> {
    let mut settings = state.settings.lock().unwrap();
    settings.game = path.clone();
    settings
        .save(&state.settings_path)
        .map_err(|err| format!("could not save the settings: {err}"))?;
    let (check, bridge) = open(path.as_deref());
    *state.bridge.write().unwrap() = bridge;
    Ok(status(path, check))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&settings_path);
            let (_, bridge) = open(settings.game.as_deref());
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
        .invoke_handler(tauri::generate_handler![game_status, set_game])
        .run(tauri::generate_context!())
        .expect("error while running kenon");
}
