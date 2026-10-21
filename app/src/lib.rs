//! The Tauri shell. It holds the settings, the open game and the open
//! project, and exposes the core to the front end as commands. It parses no
//! game formats itself.

mod protocol;
mod settings;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use kenon_core::bridge::{Bridge, Member};
use kenon_core::catalog::{self, ArchiveInfo, MemberInfo, SheetData, SoundInfo};
use kenon_core::data_root;
use kenon_core::project::{Change, Project};
use kenon_core::source::{self, GameCheck, Source};
use serde::Serialize;
use tauri::{Manager, State};

use settings::Settings;

/// How many recent projects the start screen lists.
const RECENT: usize = 8;

#[derive(Default)]
struct Open {
    /// The game, when the chosen ROM can be used.
    source: Option<Source>,
    check: Option<GameCheck>,
    project: Option<Arc<Project>>,
    /// The bridge over the game and the project.
    bridge: Option<Arc<Bridge>>,
    /// The archive listing, made once per game.
    archives: Option<Arc<Vec<ArchiveInfo>>>,
}

impl Open {
    fn rebuild_bridge(&mut self) {
        self.bridge = self
            .source
            .clone()
            .map(|s| Arc::new(Bridge::new(s).with_project(self.project.clone())));
    }
}

struct AppState {
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    open: RwLock<Open>,
}

impl AppState {
    fn bridge(&self) -> Option<Arc<Bridge>> {
        self.open.read().unwrap().bridge.clone()
    }

    fn need_bridge(&self) -> Result<Arc<Bridge>, String> {
        self.bridge()
            .ok_or_else(|| "choose your ROM first".to_string())
    }

    fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        settings
            .save(&self.settings_path)
            .map_err(|err| format!("could not save the settings: {err}"))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectInfo {
    name: String,
    root: PathBuf,
    changes: Vec<Change>,
}

/// Everything the start screen and the title bar show.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    /// The ROM (or unpacked folder) saved in the settings.
    rom: Option<PathBuf>,
    check: Option<GameCheck>,
    /// The eidos extraction folder, offered when it exists.
    suggestion: Option<PathBuf>,
    project: Option<ProjectInfo>,
    recent: Vec<PathBuf>,
}

fn status(state: &AppState) -> Status {
    let settings = state.settings.lock().unwrap();
    let open = state.open.read().unwrap();
    Status {
        rom: settings.game.clone(),
        check: open.check.clone(),
        suggestion: data_root::eidos_default().filter(|dir| dir.is_dir()),
        project: open.project.as_ref().map(|p| ProjectInfo {
            name: p.manifest.name.clone(),
            root: p.root.clone(),
            changes: p.changes(),
        }),
        recent: settings.recent.clone(),
    }
}

fn open_game(open: &mut Open, path: Option<&Path>) {
    let (check, source) = match path {
        Some(p) => {
            let (check, source) = source::open(p);
            (Some(check), source)
        }
        None => (None, None),
    };
    open.check = check;
    open.source = source;
    open.archives = None;
    open.rebuild_bridge();
}

#[tauri::command]
fn app_status(state: State<AppState>) -> Status {
    status(&state)
}

/// Saves the chosen ROM or folder (or clears it with `None`). A choice that
/// fails the check is still saved, so the user sees why.
#[tauri::command]
fn set_rom(state: State<AppState>, path: Option<PathBuf>) -> Result<Status, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.game = path.clone();
        state.save_settings(&settings)?;
    }
    open_game(&mut state.open.write().unwrap(), path.as_deref());
    Ok(status(&state))
}

fn remember(state: &AppState, root: &Path) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    settings.recent.retain(|p| p != root);
    settings.recent.insert(0, root.to_path_buf());
    settings.recent.truncate(RECENT);
    state.save_settings(&settings)
}

fn set_project(state: &AppState, project: Option<Project>) {
    let mut open = state.open.write().unwrap();
    open.project = project.map(Arc::new);
    open.rebuild_bridge();
}

/// Creates a project (a mod folder) and opens it.
#[tauri::command]
fn new_project(state: State<AppState>, root: PathBuf, name: String) -> Result<Status, String> {
    let rom = state.settings.lock().unwrap().game.clone();
    let project = Project::create(&root, &name, rom.as_deref()).map_err(|e| e.to_string())?;
    remember(&state, &root)?;
    set_project(&state, Some(project));
    Ok(status(&state))
}

/// Opens an existing mod folder.
#[tauri::command]
fn open_project(state: State<AppState>, root: PathBuf) -> Result<Status, String> {
    let project = Project::open(&root).map_err(|e| e.to_string())?;
    remember(&state, &root)?;
    set_project(&state, Some(project));
    Ok(status(&state))
}

/// Closes the project: back to the game as it is.
#[tauri::command]
fn close_project(state: State<AppState>) -> Status {
    set_project(&state, None);
    status(&state)
}

#[tauri::command]
async fn archives(state: State<'_, AppState>) -> Result<Arc<Vec<ArchiveInfo>>, String> {
    if let Some(list) = state.open.read().unwrap().archives.clone() {
        return Ok(list);
    }
    let bridge = state.need_bridge()?;
    let list = Arc::new(catalog::archives(&bridge));
    state.open.write().unwrap().archives = Some(list.clone());
    Ok(list)
}

#[tauri::command]
async fn members(state: State<'_, AppState>, arc: u16) -> Result<Vec<MemberInfo>, String> {
    let bridge = state.need_bridge()?;
    catalog::members(&bridge, arc).map_err(|e| e.to_string())
}

#[tauri::command]
async fn sounds(state: State<'_, AppState>) -> Result<Vec<SoundInfo>, String> {
    let sdat = state
        .need_bridge()?
        .sdat()
        .ok_or("the ROM has no sound archive")?;
    Ok(catalog::sounds(&sdat))
}

#[tauri::command]
async fn text_bank(
    state: State<'_, AppState>,
    arc: u16,
    bank: usize,
) -> Result<Vec<String>, String> {
    let bridge = state.need_bridge()?;
    catalog::text_bank(&bridge, arc, bank).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_text_bank(
    state: State<'_, AppState>,
    arc: u16,
    bank: usize,
    strings: Vec<String>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    catalog::save_text_bank(&bridge, arc, bank, &strings).map_err(|e| e.to_string())
}

#[tauri::command]
async fn sheet_data(
    state: State<'_, AppState>,
    chr: [usize; 2],
    pal: [usize; 2],
) -> Result<SheetData, String> {
    let bridge = state.need_bridge()?;
    catalog::sheet_data(&bridge, member(chr), member(pal)).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_sheet(
    state: State<'_, AppState>,
    chr: [usize; 2],
    tiles: Vec<Vec<u8>>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    catalog::save_sheet(&bridge, member(chr), &tiles).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_palette(
    state: State<'_, AppState>,
    pal: [usize; 2],
    colors: Vec<u32>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    catalog::save_palette(&bridge, member(pal), &colors).map_err(|e| e.to_string())
}

#[tauri::command]
async fn revert(state: State<'_, AppState>, m: [usize; 2]) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    catalog::revert(&bridge, member(m)).map_err(|e| e.to_string())
}

fn member([arc, index]: [usize; 2]) -> Member {
    Member {
        arc: arc as u16,
        index,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&settings_path);
            let mut open = Open::default();
            open_game(&mut open, settings.game.as_deref());
            app.manage(AppState {
                settings: Mutex::new(settings),
                settings_path,
                open: RwLock::new(open),
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
            app_status,
            set_rom,
            new_project,
            open_project,
            close_project,
            archives,
            members,
            sounds,
            text_bank,
            save_text_bank,
            sheet_data,
            save_sheet,
            save_palette,
            revert,
        ])
        .run(tauri::generate_context!())
        .expect("error while running kenon");
}
