//! The Tauri shell. It holds the settings, the open game and the open
//! project, and exposes the core to the front end as commands. It parses no
//! game formats itself.

mod protocol;
mod settings;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use kenon_core::bridge::{Bridge, Member};
use kenon_core::catalog::{self, ArchiveInfo, Layout, MemberInfo, SheetData, SoundInfo};
use kenon_core::data_root;
use kenon_core::modcode;
use kenon_core::project::{Change, Project};
use kenon_core::source::{self, GameCheck, Source};
use kenon_core::tables;
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

/// Packs the open project into a `.bw` file.
#[tauri::command]
fn export_bw(state: State<AppState>, path: PathBuf) -> Result<(), String> {
    let project = state.open.read().unwrap().project.clone();
    let project = project.ok_or_else(|| "open a project first".to_string())?;
    kenon_core::package::pack(&project.root, &path).map_err(|e| e.to_string())
}

/// Unpacks a `.bw` into `into_dir` and returns that folder, the new
/// project's root. The caller opens it with `open_project`.
#[tauri::command]
fn import_bw(bw_path: PathBuf, into_dir: PathBuf) -> Result<PathBuf, String> {
    kenon_core::package::unpack(&bw_path, &into_dir).map_err(|e| e.to_string())?;
    Ok(into_dir)
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
async fn members(state: State<'_, AppState>, arc: u16) -> Result<Arc<Vec<MemberInfo>>, String> {
    let bridge = state.need_bridge()?;
    catalog::members(&bridge, arc).map_err(|e| e.to_string())
}

/// The members of an archive the open project changes: cheap, unlike a
/// full listing.
#[tauri::command]
fn changed_members(state: State<AppState>, arc: u16) -> Vec<usize> {
    let open = state.open.read().unwrap();
    let Some(project) = open.project.as_ref() else {
        return Vec::new();
    };
    let Some(prefix) = kenon_core::narc::archive_path(arc) else {
        return Vec::new();
    };
    project
        .changes()
        .iter()
        .filter_map(|c| {
            c.path
                .strip_prefix(&prefix)?
                .strip_prefix('/')?
                .split('.')
                .next()?
                .parse()
                .ok()
        })
        .collect()
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
    layout: Layout,
) -> Result<SheetData, String> {
    let bridge = state.need_bridge()?;
    catalog::sheet_data(&bridge, member(chr), member(pal), &layout).map_err(|e| e.to_string())
}

/// The cell bank that assembles a sheet, if the archive has one near it.
#[tauri::command]
async fn cell_bank(state: State<'_, AppState>, chr: [usize; 2]) -> Result<Option<usize>, String> {
    let bridge = state.need_bridge()?;
    bridge.cell_bank(member(chr)).map_err(|e| e.to_string())
}

/// The background screens of an archive, each with its tiles and palette.
#[tauri::command]
async fn screens(
    state: State<'_, AppState>,
    arc: u16,
) -> Result<Arc<Vec<catalog::ScreenInfo>>, String> {
    let bridge = state.need_bridge()?;
    catalog::screens(&bridge, arc).map_err(|e| e.to_string())
}

/// The animation bank that plays a cell bank, if one is near it.
#[tauri::command]
async fn anim_bank(
    state: State<'_, AppState>,
    arc: u16,
    cer: usize,
) -> Result<Option<usize>, String> {
    let bridge = state.need_bridge()?;
    catalog::anim_bank(&bridge, arc, cer).map_err(|e| e.to_string())
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

/// Lays a PNG file over a picture and writes it into the project.
#[tauri::command]
async fn import_picture(
    state: State<'_, AppState>,
    chr: [usize; 2],
    pal: [usize; 2],
    layout: Layout,
    path: PathBuf,
    mode: kenon_core::import::PaletteMode,
    bank: Option<u8>,
) -> Result<kenon_core::import::ImportSummary, String> {
    let bridge = state.need_bridge()?;
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read the file: {e}"))?;
    kenon_core::import::import_picture(
        &bridge,
        member(chr),
        member(pal),
        &layout,
        &bytes,
        mode,
        bank.unwrap_or(0),
    )
    .map_err(|e| e.to_string())
}

/// A PNG file's bytes, for the import dialog to show it.
#[tauri::command]
async fn read_png(path: PathBuf) -> Result<tauri::ipc::Response, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read the file: {e}"))?;
    kenon_core::import::png_size(&bytes).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
async fn revert(state: State<'_, AppState>, m: [usize; 2]) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    catalog::revert(&bridge, member(m)).map_err(|e| e.to_string())
}

#[tauri::command]
async fn named_sheets(
    state: State<'_, AppState>,
    arc: u16,
) -> Result<Arc<Option<Vec<catalog::NamedSheet>>>, String> {
    let bridge = state.need_bridge()?;
    catalog::named_sheets(&bridge, arc).map_err(|e| e.to_string())
}

#[tauri::command]
async fn table_entries(
    state: State<'_, AppState>,
    table: String,
) -> Result<Arc<Vec<tables::Entry>>, String> {
    let bridge = state.need_bridge()?;
    tables::entries(&bridge, &table).map_err(|e| e.to_string())
}

#[tauri::command]
async fn table_record(
    state: State<'_, AppState>,
    table: String,
    index: usize,
    season: usize,
) -> Result<tables::Record, String> {
    let bridge = state.need_bridge()?;
    tables::record(&bridge, &table, index, season).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_record(
    state: State<'_, AppState>,
    table: String,
    index: usize,
    edit: tables::Edit,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    tables::save(&bridge, &table, index, &edit).map_err(|e| e.to_string())
}

#[tauri::command]
async fn enum_values(state: State<'_, AppState>, name: String) -> Result<Arc<Vec<String>>, String> {
    let bridge = state.need_bridge()?;
    tables::enum_values(&bridge, &name).map_err(|e| e.to_string())
}

/// Where a trainer stands in the world (zones and tiles), from the maps'
/// event data.
#[tauri::command]
async fn trainer_locations(
    state: State<'_, AppState>,
    trainer: u16,
) -> Result<Vec<kenon_core::world::Location>, String> {
    let bridge = state.need_bridge()?;
    let all = kenon_core::world::trainer_locations(&bridge).map_err(|e| e.to_string())?;
    Ok(all.get(&trainer).cloned().unwrap_or_default())
}

/// The Town Map's spots: cities, routes and places with their positions.
#[tauri::command]
async fn map_spots(
    state: State<'_, AppState>,
) -> Result<Arc<Vec<kenon_core::world::Spot>>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::world::spots(&bridge).map_err(|e| e.to_string())
}

/// The zones of a place (a town and its buildings) with their trainers.
#[tauri::command]
async fn map_place(
    state: State<'_, AppState>,
    zone: usize,
) -> Result<Vec<kenon_core::world::ZoneView>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::world::place(&bridge, zone).map_err(|e| e.to_string())
}

#[tauri::command]
async fn zone_map(
    state: State<'_, AppState>,
    zone: usize,
) -> Result<kenon_core::zonemap::ZoneMap, String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::zone_map(&bridge, zone).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_event(
    state: State<'_, AppState>,
    zone: usize,
    kind: String,
    index: usize,
    values: std::collections::HashMap<String, i64>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::save_event(&bridge, zone, &kind, index, &values).map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_event(
    state: State<'_, AppState>,
    zone: usize,
    kind: String,
    x: i32,
    z: i32,
) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::add_event(&bridge, zone, &kind, x, z).map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_event(
    state: State<'_, AppState>,
    zone: usize,
    kind: String,
    index: usize,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::remove_event(&bridge, zone, &kind, index).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_tile(
    state: State<'_, AppState>,
    zone: usize,
    x: usize,
    z: usize,
    terrain: u16,
    flags: u16,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::set_tile(&bridge, zone, x, z, terrain, flags).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_tiles(
    state: State<'_, AppState>,
    zone: usize,
    tiles: Vec<(usize, usize)>,
    terrain: u16,
    flags: u16,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::set_tiles(&bridge, zone, &tiles, terrain, flags).map_err(|e| e.to_string())
}

#[tauri::command]
async fn zones(state: State<'_, AppState>) -> Result<Arc<Vec<kenon_core::world::Zone>>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::world::zones(&bridge).map_err(|e| e.to_string())
}

#[tauri::command]
async fn describe_changes(
    state: State<'_, AppState>,
) -> Result<Vec<kenon_core::changes::ChangeInfo>, String> {
    let bridge = state.need_bridge()?;
    Ok(kenon_core::changes::describe_all(&bridge))
}

#[tauri::command]
async fn revert_change(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::changes::revert(&bridge, &path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn object_textures(state: State<'_, AppState>) -> Result<Vec<Option<u16>>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::object_textures(&bridge).map_err(|e| e.to_string())
}

#[tauri::command]
async fn event_scripts(
    state: State<'_, AppState>,
    zone: usize,
) -> Result<Vec<kenon_core::scripts::EventScript>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::scripts::event_scripts(&bridge, zone).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_event_item(
    state: State<'_, AppState>,
    script_id: u16,
    item: u16,
    count: Option<u16>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::fielditems::set_event_item(&bridge, script_id, item, count)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn free_hidden_script(state: State<'_, AppState>) -> Result<Option<u16>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::fielditems::free_hidden_script(&bridge).map_err(|e| e.to_string())
}

#[tauri::command]
async fn zone_scripts(
    state: State<'_, AppState>,
    zone: usize,
) -> Result<kenon_core::scripts::ZoneScripts, String> {
    let bridge = state.need_bridge()?;
    kenon_core::scripts::zone_scripts(&bridge, zone).map_err(|e| e.to_string())
}

#[tauri::command]
async fn props(
    state: State<'_, AppState>,
    zone: usize,
) -> Result<Vec<kenon_core::props::Prop>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::props::props(&bridge, zone).map_err(|e| e.to_string())
}

#[tauri::command]
async fn prop_models(state: State<'_, AppState>, zone: usize) -> Result<Vec<u16>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::props::prop_models(&bridge, zone).map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn set_prop(
    state: State<'_, AppState>,
    zone: usize,
    prop: kenon_core::props::Prop,
    x: f32,
    z: f32,
    y: f32,
    rot: f32,
) -> Result<(usize, usize), String> {
    let bridge = state.need_bridge()?;
    kenon_core::props::set_prop(&bridge, zone, &prop, x, z, y, rot).map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn add_prop(
    state: State<'_, AppState>,
    zone: usize,
    model: u16,
    x: f32,
    z: f32,
    y: f32,
    rot: f32,
) -> Result<(usize, usize), String> {
    let bridge = state.need_bridge()?;
    kenon_core::props::add_prop(&bridge, zone, model, x, z, y, rot).map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_prop(state: State<'_, AppState>, chunk: usize, index: usize) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::props::remove_prop(&bridge, chunk, index).map_err(|e| e.to_string())
}

#[tauri::command]
async fn trainer_lines(
    state: State<'_, AppState>,
    trainer: u16,
) -> Result<Vec<kenon_core::trainer_text::TrainerLine>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::trainer_text::trainer_lines(&bridge, trainer).map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_trainer_line(
    state: State<'_, AppState>,
    trainer: u16,
    kind: u16,
    text: String,
) -> Result<kenon_core::trainer_text::TrainerLine, String> {
    let bridge = state.need_bridge()?;
    kenon_core::trainer_text::add_trainer_line(&bridge, trainer, kind, &text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn object_sprites(
    state: State<'_, AppState>,
) -> Result<Arc<Vec<Option<kenon_core::zonemap::Sprite>>>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::object_sprites(&bridge).map_err(|e| e.to_string())
}

/// Each trainer's class, for list icons.
#[tauri::command]
async fn trainer_classes(state: State<'_, AppState>) -> Result<Vec<i64>, String> {
    let bridge = state.need_bridge()?;
    let entries = kenon_core::tables::entries(&bridge, "trainers").map_err(|e| e.to_string())?;
    Ok(entries
        .iter()
        .map(|e| {
            kenon_core::tables::record(&bridge, "trainers", e.index, 0)
                .ok()
                .and_then(|r| {
                    r.sections.iter().find_map(|s| match s {
                        kenon_core::tables::Section::Fields { values, .. } => values
                            .iter()
                            .find(|v| v.key == "trainerClass")
                            .map(|v| v.value),
                        _ => None,
                    })
                })
                .unwrap_or(-1)
        })
        .collect())
}

#[tauri::command]
async fn write_tiles(
    state: State<'_, AppState>,
    zone: usize,
    tiles: Vec<(usize, usize, u16, u16)>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::write_tiles(&bridge, zone, &tiles).map_err(|e| e.to_string())
}

#[tauri::command]
async fn move_event(
    state: State<'_, AppState>,
    zone: usize,
    kind: String,
    index: usize,
    x: i32,
    z: i32,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::move_event(&bridge, zone, &kind, index, x, z).map_err(|e| e.to_string())
}

#[tauri::command]
async fn event_record(
    state: State<'_, AppState>,
    zone: usize,
    kind: String,
    index: usize,
) -> Result<Vec<u8>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::event_record(&bridge, zone, &kind, index).map_err(|e| e.to_string())
}

#[tauri::command]
async fn insert_event(
    state: State<'_, AppState>,
    zone: usize,
    kind: String,
    index: usize,
    record: Vec<u8>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::insert_event(&bridge, zone, &kind, index, &record)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn move_events(
    state: State<'_, AppState>,
    zone: usize,
    moves: Vec<kenon_core::zonemap::EventMove>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::move_events(&bridge, zone, &moves).map_err(|e| e.to_string())
}

#[tauri::command]
async fn prop_record(
    state: State<'_, AppState>,
    chunk: usize,
    index: usize,
) -> Result<Vec<u8>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::props::prop_record(&bridge, chunk, index).map_err(|e| e.to_string())
}

#[tauri::command]
async fn insert_prop(
    state: State<'_, AppState>,
    chunk: usize,
    index: usize,
    record: Vec<u8>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::props::insert_prop(&bridge, chunk, index, &record).map_err(|e| e.to_string())
}

#[tauri::command]
async fn prop_model_names(
    state: State<'_, AppState>,
    zone: usize,
) -> Result<Vec<(u16, String)>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::props::prop_model_names(&bridge, zone).map_err(|e| e.to_string())
}

/// The type of every move, by move id.
#[tauri::command]
async fn move_types(state: State<'_, AppState>) -> Result<Vec<u8>, String> {
    let bridge = state.need_bridge()?;
    let count = bridge.base_archive(21).map_err(|e| e.to_string())?.len();
    Ok((0..count)
        .map(|index| {
            bridge
                .member(kenon_core::bridge::Member { arc: 21, index })
                .ok()
                .and_then(|b| b.first().copied())
                .unwrap_or(0)
        })
        .collect())
}

#[tauri::command]
async fn copy_event(
    state: State<'_, AppState>,
    zone: usize,
    kind: String,
    index: usize,
    x: i32,
    z: i32,
) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::copy_event(&bridge, zone, &kind, index, x, z).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_trainer(
    state: State<'_, AppState>,
    zone: usize,
    index: usize,
    trainer: Option<u16>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::set_trainer(&bridge, zone, index, trainer).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_music(state: State<'_, AppState>, zone: usize, music: [u16; 4]) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::zonemap::set_music(&bridge, zone, music).map_err(|e| e.to_string())
}

// The mod code editors (core/src/modcode.rs). Each command that changes the
// manifest reopens the project, so the copy the app holds matches the disk.

fn mod_root(state: &AppState) -> Result<PathBuf, String> {
    let open = state.open.read().unwrap();
    open.project
        .as_ref()
        .map(|p| p.root.clone())
        .ok_or_else(|| "open a project first".to_string())
}

fn reload_project(state: &AppState) -> Result<(), String> {
    let root = mod_root(state)?;
    let project = Project::open(&root).map_err(|e| e.to_string())?;
    set_project(state, Some(project));
    Ok(())
}

/// The manifest form, how many hook lines the mod has, and the api the port
/// accepts.
#[tauri::command]
fn mod_get_settings(state: State<AppState>) -> Result<(modcode::ModSettings, usize, u32), String> {
    let root = mod_root(&state)?;
    let settings = modcode::read_settings(&root).map_err(|e| e.to_string())?;
    let hooks = modcode::read_overrides(&root).map_err(|e| e.to_string())?;
    Ok((settings, hooks.lines().count(), modcode::API_VERSION))
}

#[tauri::command]
fn mod_check_settings(
    state: State<AppState>,
    settings: modcode::ModSettings,
) -> Result<Vec<modcode::Diagnostic>, String> {
    let root = mod_root(&state)?;
    let hooks = modcode::read_overrides(&root).map_err(|e| e.to_string())?;
    Ok(modcode::check_settings(&settings, hooks.lines().count()))
}

#[tauri::command]
fn mod_save_settings(state: State<AppState>, settings: modcode::ModSettings) -> Result<(), String> {
    let root = mod_root(&state)?;
    modcode::write_settings(&root, &settings).map_err(|e| e.to_string())?;
    reload_project(&state)
}

/// The declared set: the chosen file, else the one beside the mods folder.
fn declared_set(state: &AppState) -> Option<modcode::DeclaredSet> {
    let settings = state.settings.lock().unwrap();
    let path = settings.declared_set.clone().or_else(|| {
        settings
            .mods_folder
            .as_deref()
            .and_then(modcode::declared_set_near)
    })?;
    modcode::DeclaredSet::load(&path).ok()
}

#[tauri::command]
fn mod_get_hooks(state: State<AppState>) -> Result<String, String> {
    let root = mod_root(&state)?;
    modcode::read_overrides(&root).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_check_hooks(state: State<AppState>, text: String) -> modcode::HooksCheck {
    modcode::check_overrides(&text, declared_set(&state).as_ref())
}

#[tauri::command]
fn mod_save_hooks(state: State<AppState>, text: String) -> Result<(), String> {
    let root = mod_root(&state)?;
    modcode::write_overrides(&root, &text).map_err(|e| e.to_string())?;
    reload_project(&state)
}

/// The hookable functions as `[region, address, function, events]`, for
/// completion; empty when no declared set is found.
#[tauri::command]
fn mod_declared(state: State<AppState>) -> Vec<(String, String, String, Vec<String>)> {
    declared_set(&state)
        .map(|set| {
            set.rows()
                .into_iter()
                .map(|(region, addr, row)| {
                    (
                        region,
                        format!("0x{addr:08X}"),
                        row.function.clone(),
                        row.events.clone(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command]
fn mod_rule_names() -> &'static modcode::Names {
    modcode::rule_names()
}

#[tauri::command]
fn mod_list_rules(state: State<AppState>) -> Result<Vec<modcode::RuleEntry>, String> {
    let root = mod_root(&state)?;
    modcode::list_rules(&root).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_read_rule(state: State<AppState>, path: String) -> Result<String, String> {
    let root = mod_root(&state)?;
    modcode::read_rule(&root, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_check_rule(text: String, event: Option<String>) -> Vec<modcode::Diagnostic> {
    modcode::check_rule(&text, event.as_deref())
}

#[tauri::command]
fn mod_save_rule(
    state: State<AppState>,
    path: String,
    text: String,
) -> Result<Vec<modcode::Diagnostic>, String> {
    let root = mod_root(&state)?;
    let found = modcode::write_rule(&root, &path, &text).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(found)
}

#[tauri::command]
fn mod_new_rule(state: State<AppState>, name: String, event: String) -> Result<String, String> {
    let root = mod_root(&state)?;
    let path = modcode::new_rule(&root, &name, &event).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(path)
}

#[tauri::command]
fn mod_rename_rule(state: State<AppState>, from: String, to: String) -> Result<String, String> {
    let root = mod_root(&state)?;
    let path = modcode::rename_rule(&root, &from, &to).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(path)
}

#[tauri::command]
fn mod_delete_rule(state: State<AppState>, path: String) -> Result<(), String> {
    let root = mod_root(&state)?;
    modcode::delete_rule(&root, &path).map_err(|e| e.to_string())?;
    reload_project(&state)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModPaths {
    mods_folder: Option<PathBuf>,
    declared_set: Option<PathBuf>,
    /// How many functions the declared set in use lists, if one is found.
    declared_found: Option<usize>,
}

fn mod_paths(state: &AppState) -> ModPaths {
    let settings = state.settings.lock().unwrap().clone();
    ModPaths {
        mods_folder: settings.mods_folder,
        declared_set: settings.declared_set,
        declared_found: declared_set(state).map(|s| s.len()),
    }
}

#[tauri::command]
fn mod_get_paths(state: State<AppState>) -> ModPaths {
    mod_paths(&state)
}

#[tauri::command]
fn mod_set_paths(
    state: State<AppState>,
    mods_folder: Option<PathBuf>,
    declared_set: Option<PathBuf>,
) -> Result<ModPaths, String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.mods_folder = mods_folder;
        settings.declared_set = declared_set;
        state.save_settings(&settings)?;
    }
    Ok(mod_paths(&state))
}

#[tauri::command]
fn mod_conflicts(state: State<AppState>) -> Result<modcode::ConflictReport, String> {
    let root = mod_root(&state)?;
    let folder = state
        .settings
        .lock()
        .unwrap()
        .mods_folder
        .clone()
        .ok_or_else(|| "choose the mods folder first".to_string())?;
    modcode::find_conflicts(&root, &folder)
        .map_err(|e| format!("could not read the mods folder: {e}"))
}

/// Writes the bytes of a bridge asset (a picture, a sound, JSON) to a file.
#[tauri::command]
async fn export_asset(
    state: State<'_, AppState>,
    path: String,
    dest: PathBuf,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let asset = bridge.serve(&path).map_err(|e| e.to_string())?;
    write_out(&dest, &asset.bytes)
}

/// Writes a game file to a file: decompressed, or as stored when `raw`.
#[tauri::command]
async fn export_member(
    state: State<'_, AppState>,
    arc: u16,
    index: usize,
    dest: PathBuf,
    raw: bool,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let m = Member { arc, index };
    let bytes = if raw {
        bridge.member_raw(m)
    } else {
        bridge.member(m)
    };
    write_out(&dest, &bytes.map_err(|e| e.to_string())?)
}

/// Writes text to a file.
#[tauri::command]
fn export_text(dest: PathBuf, text: String) -> Result<(), String> {
    write_out(&dest, text.as_bytes())
}

/// The archive a table's records live in.
#[tauri::command]
fn table_archive(table: String) -> Result<u16, String> {
    tables::table(&table)
        .map(|t| t.archive)
        .map_err(|e| e.to_string())
}

fn write_out(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(dest, bytes).map_err(|e| format!("could not write {}: {e}", dest.display()))
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
        // Keep where the window is and whether it is maximized, so the next
        // start opens it the same way at once.
        .on_window_event(|window, event| {
            if !matches!(
                event,
                tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::Moved(_)
                    | tauri::WindowEvent::CloseRequested { .. }
            ) {
                return;
            }
            let Some(state) = window.try_state::<AppState>() else {
                return;
            };
            let maximized = window.is_maximized().unwrap_or(false);
            let minimized = window.is_minimized().unwrap_or(false);
            let scale = window.scale_factor().unwrap_or(1.0);
            let mut settings = state.settings.lock().unwrap();
            let mut place = settings.window.clone().unwrap_or_default();
            if !minimized {
                place.maximized = maximized;
                if !maximized {
                    if let (Ok(size), Ok(pos)) = (window.inner_size(), window.outer_position()) {
                        place.width = size.width as f64 / scale;
                        place.height = size.height as f64 / scale;
                        place.x = pos.x as f64 / scale;
                        place.y = pos.y as f64 / scale;
                    }
                }
            }
            if settings.window.as_ref() != Some(&place) {
                settings.window = Some(place);
                let _ = settings.save(&state.settings_path);
            }
        })
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
            // The window is made here, already at its last size and place and
            // already maximized when it was, so it opens in one step.
            let place = app
                .state::<AppState>()
                .settings
                .lock()
                .unwrap()
                .window
                .clone();
            let mut window =
                tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
                    .title("kenon")
                    .min_inner_size(640.0, 480.0);
            window = match place.filter(|p| p.width >= 640.0 && p.height >= 480.0) {
                Some(p) => window
                    .inner_size(p.width, p.height)
                    .position(p.x, p.y)
                    .maximized(p.maximized),
                None => window.inner_size(1024.0, 720.0).center(),
            };
            // A window made in code takes the app icon only when given it.
            if let Some(icon) = app.default_window_icon() {
                window = window.icon(icon.clone())?;
            }
            window.build()?;
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
            export_bw,
            import_bw,
            archives,
            export_asset,
            export_member,
            export_text,
            table_archive,
            members,
            changed_members,
            sounds,
            text_bank,
            save_text_bank,
            sheet_data,
            cell_bank,
            screens,
            anim_bank,
            save_sheet,
            save_palette,
            import_picture,
            read_png,
            revert,
            named_sheets,
            table_entries,
            table_record,
            save_record,
            enum_values,
            trainer_locations,
            map_spots,
            map_place,
            zone_map,
            save_event,
            add_event,
            remove_event,
            set_tile,
            set_tiles,
            write_tiles,
            trainer_classes,
            object_sprites,
            trainer_lines,
            add_trainer_line,
            props,
            prop_models,
            set_prop,
            add_prop,
            remove_prop,
            event_scripts,
            set_event_item,
            free_hidden_script,
            zone_scripts,
            object_textures,
            describe_changes,
            revert_change,
            zones,
            move_event,
            copy_event,
            event_record,
            insert_event,
            move_events,
            prop_record,
            insert_prop,
            prop_model_names,
            move_types,
            set_trainer,
            set_music,
            mod_get_settings,
            mod_check_settings,
            mod_save_settings,
            mod_get_hooks,
            mod_check_hooks,
            mod_save_hooks,
            mod_declared,
            mod_rule_names,
            mod_list_rules,
            mod_read_rule,
            mod_check_rule,
            mod_save_rule,
            mod_new_rule,
            mod_rename_rule,
            mod_delete_rule,
            mod_get_paths,
            mod_set_paths,
            mod_conflicts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running kenon");
}
