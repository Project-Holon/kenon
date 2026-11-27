//! The Tauri shell. It holds the settings, the open game and the open
//! project, and exposes the core to the front end as commands. It parses no
//! game formats itself.

mod protocol;
mod settings;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use kenon_core::bridge::{Bridge, Member};
use kenon_core::catalog::{self, ArchiveInfo, Layout, MemberInfo, SheetData, SoundInfo};
use kenon_core::codemod;
use kenon_core::data_root;
use kenon_core::modcode;
use kenon_core::project::{Change, Project};
use kenon_core::source::{self, GameCheck, Source};
use kenon_core::tables;
use serde::Serialize;
use tauri::{Emitter, Manager, State};

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
    /// Reference ROMs opened for their text, by language.
    references: Mutex<std::collections::HashMap<String, Arc<Bridge>>>,
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
    /// Whether the ROM matched a known good dump (None: not checked).
    rom_verified: Option<bool>,
}

/// A recent project as the start screen shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecentProject {
    root: PathBuf,
    name: String,
    description: String,
    /// The game code the mod is made for, empty when it does not say.
    game: String,
    changes: usize,
    /// Seconds since 1970, 0 when not known.
    opened: u64,
    /// The folder is gone.
    missing: bool,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
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
        rom_verified: settings.rom_verified,
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
    // Hashing the whole ROM takes a moment, so it is done once, here.
    let verified = match &state.open.read().unwrap().source {
        Some(Source::Rom(rom)) => rom.is_known_dump().ok(),
        _ => None,
    };
    {
        let mut settings = state.settings.lock().unwrap();
        settings.rom_verified = verified;
        state.save_settings(&settings)?;
    }
    Ok(status(&state))
}

/// The recent projects with what the start screen shows of each.
#[tauri::command]
fn recent_projects(state: State<AppState>) -> Vec<RecentProject> {
    let settings = state.settings.lock().unwrap().clone();
    settings
        .recent
        .iter()
        .map(|root| {
            let opened = settings.opened.get(root).copied().unwrap_or(0);
            match Project::open(root) {
                Ok(p) => RecentProject {
                    root: root.clone(),
                    name: p.manifest.name.clone(),
                    description: p.manifest.description.clone(),
                    game: p.manifest.game.clone(),
                    changes: p.changes().len(),
                    opened,
                    missing: false,
                },
                Err(_) => RecentProject {
                    root: root.clone(),
                    name: root
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    description: String::new(),
                    game: String::new(),
                    changes: 0,
                    opened,
                    missing: true,
                },
            }
        })
        .collect()
}

/// Drops a project from the recent list (its folder is left alone).
#[tauri::command]
fn forget_recent(state: State<AppState>, root: PathBuf) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    settings.recent.retain(|p| p != &root);
    settings.opened.remove(&root);
    state.save_settings(&settings)
}

fn remember(state: &AppState, root: &Path) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    settings.recent.retain(|p| p != root);
    settings.recent.insert(0, root.to_path_buf());
    settings.recent.truncate(RECENT);
    settings.opened.insert(root.to_path_buf(), now_secs());
    let kept: Vec<PathBuf> = settings.recent.clone();
    settings.opened.retain(|p, _| kept.contains(p));
    state.save_settings(&settings)
}

fn set_project(state: &AppState, project: Option<Project>) {
    let mut open = state.open.write().unwrap();
    open.project = project.map(Arc::new);
    open.rebuild_bridge();
}

/// Creates a project (a mod folder) and opens it.
#[tauri::command]
fn new_project(
    state: State<AppState>,
    root: PathBuf,
    name: String,
    description: Option<String>,
    author: Option<String>,
    version: Option<String>,
    game: Option<String>,
) -> Result<Status, String> {
    // `black` or `white` limits the mod to that game; none is both.
    let game = game.unwrap_or_default();
    kenon_core::project::game_versions(&game)?;
    let rom = state.settings.lock().unwrap().game.clone();
    let mut project = Project::create(&root, &name, rom.as_deref()).map_err(|e| e.to_string())?;
    project.manifest.description = description.unwrap_or_default();
    project.manifest.author = author.unwrap_or_default();
    project.manifest.version = version.unwrap_or_default();
    project.manifest.game = game;
    project.save_meta().map_err(|e| e.to_string())?;
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

/// The open ROM's language and the reference ROMs set, by language.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Languages {
    base: Option<&'static str>,
    references: std::collections::BTreeMap<String, PathBuf>,
}

#[tauri::command]
fn languages(state: State<AppState>) -> Result<Languages, String> {
    let base = match &state.open.read().unwrap().source {
        Some(kenon_core::source::Source::Rom(rom)) => kenon_core::reference::language_of(rom),
        _ => None,
    };
    let references = state.settings.lock().unwrap().reference_roms.clone();
    Ok(Languages { base, references })
}

/// Sets a ROM as the reference for its language (read from the ROM).
/// Returns the language.
#[tauri::command]
fn set_reference_rom(state: State<AppState>, path: PathBuf) -> Result<String, String> {
    let rom =
        kenon_core::rom::Rom::open(&path).map_err(|e| format!("could not read the ROM: {e}"))?;
    let lang = kenon_core::reference::language_of(&rom).ok_or(
        "Unsupported ROM language. eidos supports English, French, German, Italian and Spanish.",
    )?;
    let mut settings = state.settings.lock().unwrap();
    settings.reference_roms.insert(lang.to_string(), path);
    state.save_settings(&settings)?;
    state.references.lock().unwrap().remove(lang);
    Ok(lang.to_string())
}

#[tauri::command]
fn clear_reference_rom(state: State<AppState>, lang: String) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    settings.reference_roms.remove(&lang);
    state.save_settings(&settings)?;
    state.references.lock().unwrap().remove(&lang);
    Ok(())
}

/// The game's own text of a bank in `lang`: from its reference ROM when one
/// is set, else kenon's bundled names and descriptions.
#[tauri::command]
fn reference_text(
    state: State<AppState>,
    lang: String,
    arc: u16,
    bank: usize,
) -> Result<Option<kenon_core::reference::Lines>, String> {
    let path = state
        .settings
        .lock()
        .unwrap()
        .reference_roms
        .get(&lang)
        .cloned();
    if let Some(path) = path {
        let mut refs = state.references.lock().unwrap();
        if !refs.contains_key(&lang) {
            let rom = kenon_core::rom::Rom::open(&path)
                .map_err(|e| format!("could not read the reference ROM: {e}"))?;
            let source = kenon_core::source::Source::Rom(Arc::new(rom));
            refs.insert(lang.clone(), Arc::new(Bridge::new(source)));
        }
        let bridge = refs[&lang].clone();
        drop(refs);
        return kenon_core::reference::rom_lines(&bridge, arc, bank)
            .map(Some)
            .map_err(|e| e.to_string());
    }
    Ok((arc == 2)
        .then(|| kenon_core::reference::bundled_lines(&lang, bank))
        .flatten())
}

/// The mod's lines of a bank in `lang` (`<bank>.<lang>.msg`).
#[tauri::command]
fn mod_lang_lines(
    state: State<AppState>,
    arc: u16,
    bank: usize,
    lang: String,
) -> Result<std::collections::BTreeMap<usize, String>, String> {
    let project = state
        .open
        .read()
        .unwrap()
        .project
        .clone()
        .ok_or("open a project first")?;
    Ok(project.msg_lines_in(arc, bank, Some(&lang)))
}

/// Sets lines of a bank in `lang`; an empty text removes a line.
#[tauri::command]
fn set_mod_lang_lines(
    state: State<AppState>,
    arc: u16,
    bank: usize,
    lang: String,
    lines: Vec<(usize, String)>,
) -> Result<(), String> {
    let project = state
        .open
        .read()
        .unwrap()
        .project
        .clone()
        .ok_or("open a project first")?;
    kenon_core::translations::set(&project, arc, bank, &lang, &lines).map_err(|e| e.to_string())
}

/// Official names in `lang` for the mod's untranslated names: (bank,
/// line, name).
#[tauri::command]
fn official_names(
    state: State<AppState>,
    lang: String,
) -> Result<Vec<(usize, usize, String)>, String> {
    let project = state
        .open
        .read()
        .unwrap()
        .project
        .clone()
        .ok_or("open a project first")?;
    Ok(kenon_core::translations::official_names(&project, &lang))
}

/// A text file a translator filled (UTF-8).
#[tauri::command]
fn import_text(path: PathBuf) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("could not read the file: {e}"))
}

/// Sets one translation (game text, the mod's name or description, a
/// string, an interface word); an empty text removes it.
#[tauri::command]
fn set_translation(
    state: State<AppState>,
    lang: String,
    source: String,
    key: String,
    text: String,
) -> Result<(), String> {
    let project = state
        .open
        .read()
        .unwrap()
        .project
        .clone()
        .ok_or("open a project first")?;
    kenon_core::translations::set_one(&project, &lang, &source, &key, &text)
        .map_err(|e| e.to_string())?;
    reload_project(&state)
}

/// Every line the mod sets, with its translation in `lang`.
#[tauri::command]
fn translation_rows(
    state: State<AppState>,
    lang: String,
) -> Result<Vec<kenon_core::translations::Row>, String> {
    let project = state
        .open
        .read()
        .unwrap()
        .project
        .clone()
        .ok_or("open a project first")?;
    Ok(kenon_core::translations::rows(&project, &lang))
}

/// Packs the open project into a `.bw` file.
#[tauri::command]
fn export_bw(state: State<AppState>, path: PathBuf) -> Result<(), String> {
    let project = state.open.read().unwrap().project.clone();
    let project = project.ok_or_else(|| "open a project first".to_string())?;
    set_mod_language(&state, &project)?;
    refresh_min_eidos(&project)?;
    kenon_core::package::pack(&project.root, &path).map_err(|e| e.to_string())
}

/// The mod's `lang` line: the language of the ROM its text was made on.
fn set_mod_language(state: &AppState, project: &Project) -> Result<(), String> {
    let lang = match &state.open.read().unwrap().source {
        Some(kenon_core::source::Source::Rom(rom)) => kenon_core::reference::language_of(rom),
        _ => None,
    };
    kenon_core::text_from::set_language(project, lang).map_err(|e| e.to_string())
}

/// Writes the mod's `eidos = ` line for what it uses (crate min_eidos).
fn refresh_min_eidos(project: &Project) -> Result<kenon_core::min_eidos::Need, String> {
    let features =
        kenon_core::min_eidos::Features::load(kenon_core::data_root::eidos_features().as_deref())?;
    kenon_core::min_eidos::refresh(project, &features)
}

/// The oldest eidos the open mod works on, and what needs it.
#[tauri::command]
fn mod_min_eidos(state: State<AppState>) -> Result<kenon_core::min_eidos::Need, String> {
    let project = state.open.read().unwrap().project.clone();
    let project = project.ok_or_else(|| "open a project first".to_string())?;
    let need = refresh_min_eidos(&project)?;
    reload_project(&state)?;
    Ok(need)
}

/// Unpacks a `.bw` into `into_dir` and returns that folder, the new
/// project's root. The caller opens it with `open_project`.
#[tauri::command]
fn import_bw(bw_path: PathBuf, into_dir: PathBuf) -> Result<PathBuf, String> {
    kenon_core::package::unpack(&bw_path, &into_dir).map_err(|e| e.to_string())?;
    Ok(into_dir)
}

/// Makes a project in `into_dir` from a ROM hack patch (xdelta), applied to
/// the open ROM. The caller shows the report and opens the project with
/// `open_project`.
#[tauri::command]
async fn import_patch(
    state: State<'_, AppState>,
    patch_path: PathBuf,
    into_dir: PathBuf,
    name: String,
) -> Result<kenon_core::romhack::PatchImport, String> {
    let rom = state.settings.lock().unwrap().game.clone();
    let rom = rom.filter(|p| p.is_file()).ok_or_else(|| {
        "a patch needs the ROM it was made for: choose a ROM file first".to_string()
    })?;
    tauri::async_runtime::spawn_blocking(move || {
        kenon_core::romhack::import_patch(&rom, &patch_path, &into_dir, &name)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
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

/// Puts a WAV file in place of a Pokemon's cry.
#[tauri::command]
async fn import_cry(
    state: State<'_, AppState>,
    species: u32,
    path: PathBuf,
) -> Result<kenon_core::audio_import::AudioSummary, String> {
    let bridge = state.need_bridge()?;
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read the file: {e}"))?;
    kenon_core::audio_import::import_cry(&bridge, species, &bytes).map_err(|e| e.to_string())
}

/// Puts a WAV file in place of the wave a sound plays.
#[tauri::command]
async fn import_sound(
    state: State<'_, AppState>,
    id: u32,
    path: PathBuf,
) -> Result<kenon_core::audio_import::AudioSummary, String> {
    let bridge = state.need_bridge()?;
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read the file: {e}"))?;
    kenon_core::audio_import::import_sound(&bridge, id, &bytes).map_err(|e| e.to_string())
}

/// Puts the game's cry back.
#[tauri::command]
async fn revert_cry(state: State<'_, AppState>, species: u32) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::audio_import::revert_cry(&bridge, species).map_err(|e| e.to_string())
}

/// Puts the game's wave back in a sound.
#[tauri::command]
async fn revert_sound(state: State<'_, AppState>, id: u32) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::audio_import::revert_sound(&bridge, id).map_err(|e| e.to_string())
}

/// The cries and sounds the project has replaced.
#[tauri::command]
async fn audio_changes(
    state: State<'_, AppState>,
) -> Result<kenon_core::audio_import::AudioChanges, String> {
    let bridge = state.need_bridge()?;
    kenon_core::audio_import::changes(&bridge).map_err(|e| e.to_string())
}

/// Adds a WAV file to the sound archive as a new sound effect, or as a
/// music loop.
#[tauri::command]
async fn add_sound(
    state: State<'_, AppState>,
    name: String,
    path: PathBuf,
    music: bool,
) -> Result<kenon_core::sound_add::NewSound, String> {
    let bridge = state.need_bridge()?;
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read the file: {e}"))?;
    if music {
        kenon_core::sound_add::add_music(&bridge, &name, &bytes)
    } else {
        kenon_core::sound_add::add_sound_effect(&bridge, &name, &bytes)
    }
    .map_err(|e| e.to_string())
}

/// Adds a character that looks like `like`, with its pictures from PNG files.
#[tauri::command]
async fn add_character(
    state: State<'_, AppState>,
    paths: Vec<PathBuf>,
    like: u16,
) -> Result<kenon_core::characters::NewCharacter, String> {
    let bridge = state.need_bridge()?;
    let frames = paths
        .iter()
        .map(|p| std::fs::read(p).map_err(|e| format!("could not read {}: {e}", p.display())))
        .collect::<Result<Vec<_>, _>>()?;
    kenon_core::characters::add_character(&bridge, &frames, like).map_err(|e| e.to_string())
}

/// The characters added to the project.
#[tauri::command]
async fn added_characters(
    state: State<'_, AppState>,
) -> Result<Vec<kenon_core::characters::NewCharacter>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::characters::added_characters(&bridge).map_err(|e| e.to_string())
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

/// The battle code a move, ability or item has (kenon_core::battle).
#[tauri::command]
async fn battle_coded(
    state: State<'_, AppState>,
    owner: String,
    id: u16,
) -> Result<Option<kenon_core::battle::Coded>, String> {
    let bridge = state.need_bridge()?;
    let owner = kenon_core::battle::Owner::parse(&owner).ok_or("no such owner")?;
    kenon_core::battle::coded_of(&bridge, owner, id).map_err(|e| e.to_string())
}

/// Every id of an owner that has battle code.
#[tauri::command]
async fn battle_coded_all(
    state: State<'_, AppState>,
    owner: String,
) -> Result<Arc<Vec<kenon_core::battle::Coded>>, String> {
    let bridge = state.need_bridge()?;
    let owner = kenon_core::battle::Owner::parse(&owner).ok_or("no such owner")?;
    kenon_core::battle::coded(&bridge, owner).map_err(|e| e.to_string())
}

#[tauri::command]
async fn type_chart(state: State<'_, AppState>) -> Result<Vec<Vec<u8>>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::types::chart(&bridge).map_err(|e| e.to_string())
}

/// Writes the type chart (eidos type_chart.txt).
#[tauri::command]
fn set_type_chart(state: State<AppState>, chart: Vec<Vec<u8>>) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::types::set_chart(&bridge, &chart).map_err(|e| e.to_string())
}

/// A type added: its id, and why its pictures could not be made (a letter
/// no game icon has), if so.
#[derive(Serialize)]
struct AddedType {
    id: usize,
    note: Option<String>,
}

/// Adds a type matching up like `from`, with its pictures made from
/// `from`'s.
#[tauri::command]
fn add_type(state: State<AppState>, name: String, from: usize) -> Result<AddedType, String> {
    let bridge = state.need_bridge()?;
    let id = kenon_core::types::add(&bridge, &name, from).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    let bridge = state.need_bridge()?;
    let note = kenon_core::type_pictures::create(&bridge, id, from, &name, None)
        .err()
        .map(|e| e.to_string());
    reload_project(&state)?;
    Ok(AddedType { id, note })
}

/// An added type's pictures (None: a game type, or none made yet).
#[tauri::command]
fn type_look(
    state: State<AppState>,
    id: usize,
) -> Result<Option<kenon_core::type_pictures::TypeLook>, String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    Ok(kenon_core::type_pictures::all(project).remove(&id))
}

/// Remakes an added type's pictures from a game type (and its name).
#[tauri::command]
fn remake_type_pictures(state: State<AppState>, id: usize, from: usize) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let name = kenon_core::tables::enum_values(&bridge, "type")
        .map_err(|e| e.to_string())?
        .get(id)
        .cloned()
        .unwrap_or_default();
    kenon_core::type_pictures::create(&bridge, id, from, &name, None).map_err(|e| e.to_string())?;
    Ok(())
}

/// Adds "<Type> Plate" for an added type and makes it the type's plate.
#[tauri::command]
fn add_type_plate(state: State<AppState>, id: usize) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    let mut params = kenon_core::handlers::params(
        "item",
        kenon_core::data_root::eidos_handler_params().as_deref(),
    )?;
    if let Some(project) = bridge.project() {
        params.add_types(&mod_type_words(project));
    }
    let item =
        kenon_core::type_items::add_plate(&bridge, id, &params).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(item)
}

/// Adds "<Type> Drive" for an added type and a Genesect form for it (a
/// copy of the Douse Drive form to recolour).
#[tauri::command]
fn add_type_drive(state: State<AppState>, id: usize) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    let form =
        kenon_core::species_add::add_game_form(&bridge, GENESECT, 1).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    let bridge = state.need_bridge()?;
    let item = kenon_core::type_items::add_drive(&bridge, id, form).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(item)
}

const ARCEUS: usize = 493;
const GENESECT: usize = 649;

/// How many forms a game Pokemon has in this mod (its own included).
#[tauri::command]
fn game_form_count(state: State<AppState>, species: usize) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    let rec = bridge
        .member(kenon_core::bridge::Member {
            arc: 16,
            index: species,
        })
        .map_err(|e| e.to_string())?;
    Ok(rec.get(32).copied().unwrap_or(1) as usize)
}

/// Adds Arceus's form for an added type: form number = the type's, made
/// from Arceus's own colours.
#[tauri::command]
fn add_arceus_form(state: State<AppState>, id: usize) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    let count = game_form_count(state.clone(), ARCEUS)?;
    if count != id {
        return Err(format!(
            "Arceus has {count} forms: add the forms of the types before this one first"
        ));
    }
    let form =
        kenon_core::species_add::add_game_form(&bridge, ARCEUS, 0).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(form)
}

/// Sets or clears an added type's plate (an item).
#[tauri::command]
fn set_type_plate(state: State<AppState>, id: usize, item: Option<usize>) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::type_items::set_plate(&bridge, id, item).map_err(|e| e.to_string())
}

/// Sets or clears an added type's drive (an item and a Genesect form).
#[tauri::command]
fn set_type_drive(
    state: State<AppState>,
    id: usize,
    drive: Option<(usize, usize)>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::type_items::set_drive(&bridge, id, drive).map_err(|e| e.to_string())
}

/// How much HP an item heals: from use and held (record and item_heal.txt).
#[tauri::command]
fn item_heal(state: State<AppState>, item: usize) -> Result<kenon_core::item_heal::Heal, String> {
    let bridge = state.need_bridge()?;
    kenon_core::item_heal::get(&bridge, item).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_item_heal(
    state: State<AppState>,
    item: usize,
    heal: kenon_core::item_heal::Heal,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::item_heal::set(&bridge, item, heal).map_err(|e| e.to_string())
}

/// An item's Poke Ball line and the keys its base takes, when it is a ball.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BallView {
    ball: kenon_core::balls::Ball,
    keys: Vec<&'static str>,
    /// The game's ball items: (item, ball id).
    bases: Vec<(u16, u16)>,
}

#[tauri::command]
fn item_ball(state: State<AppState>, item: usize) -> Result<Option<BallView>, String> {
    let bridge = state.need_bridge()?;
    let Some(ball) = kenon_core::balls::get(&bridge, item) else {
        return Ok(None);
    };
    let game = kenon_core::balls::game_balls(&bridge);
    let keys = game
        .get(&ball.base)
        .map(|&b| kenon_core::balls::keys_for(b))
        .unwrap_or_default();
    Ok(Some(BallView {
        ball,
        keys,
        bases: game.into_iter().collect(),
    }))
}

#[tauri::command]
fn set_item_ball(
    state: State<AppState>,
    item: usize,
    base: u16,
    keys: std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::balls::set(&bridge, item, base, keys).map_err(|e| e.to_string())
}

/// A move's kinds (bite, pulse, ball, powder) in this mod.
#[tauri::command]
fn move_kinds(state: State<AppState>, id: usize) -> Result<Vec<String>, String> {
    let project = state
        .open
        .read()
        .unwrap()
        .project
        .clone()
        .ok_or("open a project first")?;
    Ok(kenon_core::move_kinds::kinds(&project, id))
}

#[tauri::command]
fn set_move_kinds(state: State<AppState>, id: usize, kinds: Vec<String>) -> Result<(), String> {
    let project = state
        .open
        .read()
        .unwrap()
        .project
        .clone()
        .ok_or("open a project first")?;
    kenon_core::move_kinds::set(&project, id, &kinds).map_err(|e| e.to_string())
}

/// Sets an added type's colours (RRGGBB).
#[tauri::command]
fn set_type_colors(
    state: State<AppState>,
    id: usize,
    fill: String,
    border: String,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::type_pictures::set_colors(&bridge, id, &fill, &border).map_err(|e| e.to_string())
}

/// Replaces an added type's icon from a 32 x 16 PNG.
#[tauri::command]
fn import_type_icon(state: State<AppState>, id: usize, path: PathBuf) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read the file: {e}"))?;
    let img = kenon_core::import::decode_png(&bytes).map_err(|e| e.to_string())?;
    if (img.width, img.height) != (32, 16) {
        return Err(format!(
            "a type icon is 32 x 16 pixels; this one is {} x {}",
            img.width, img.height
        ));
    }
    kenon_core::type_pictures::import_icon(&bridge, id, &img.pixels).map_err(|e| e.to_string())
}

/// Removes a type the mod adds; later types move down one.
#[tauri::command]
fn remove_type(state: State<AppState>, id: usize) -> Result<kenon_core::content::Removed, String> {
    let bridge = state.need_bridge()?;
    let r = kenon_core::types::remove(&bridge, id).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(r)
}

/// How many types the mod has.
#[tauri::command]
fn type_count(state: State<AppState>) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    Ok(kenon_core::types::count(&bridge))
}

/// The particle files, with the moves that load them.
#[tauri::command]
async fn particle_list(state: State<'_, AppState>) -> Result<Vec<kenon_core::spa::Listed>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::spa::list(&bridge).map_err(|e| e.to_string())
}

/// The editable fields of a particle emitter (the same for every file).
#[tauri::command]
fn particle_fields() -> &'static [kenon_core::spa::Field] {
    kenon_core::spa::FIELDS
}

/// One particle file: its emitters' values, its textures, its problems.
#[tauri::command]
fn particle_file(
    state: State<AppState>,
    index: usize,
) -> Result<kenon_core::spa::FileView, String> {
    let bridge = state.need_bridge()?;
    let s = kenon_core::spa::load(&bridge, index).map_err(|e| e.to_string())?;
    Ok(kenon_core::spa::view(&s))
}

/// Saves edits to a particle file: blocks switched on or off, then values.
#[tauri::command]
fn set_particle_file(
    state: State<AppState>,
    index: usize,
    blocks: Vec<(usize, String, bool)>,
    edits: Vec<kenon_core::spa::Edit>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let mut s = kenon_core::spa::load(&bridge, index).map_err(|e| e.to_string())?;
    kenon_core::spa::apply(&mut s, &blocks, &edits).map_err(|e| e.to_string())?;
    kenon_core::spa::store(&bridge, index, &s).map_err(|e| e.to_string())
}

/// A new particle file, a copy of `from`; returns its number.
#[tauri::command]
fn add_particle_file(state: State<AppState>, from: usize) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    kenon_core::spa::add(&bridge, from).map_err(|e| e.to_string())
}

/// Replaces a particle texture from a PNG, in the texture's own format.
#[tauri::command]
fn import_particle_texture(
    state: State<AppState>,
    index: usize,
    texture: usize,
    path: PathBuf,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read the file: {e}"))?;
    let img = kenon_core::import::decode_png(&bytes).map_err(|e| e.to_string())?;
    let mut s = kenon_core::spa::load(&bridge, index).map_err(|e| e.to_string())?;
    let old = s.textures.get(texture).ok_or("no such texture")?.clone();
    s.textures[texture] = kenon_core::spa::encode_texture(&old, img.width, img.height, &img.pixels)
        .map_err(|e| e.to_string())?;
    kenon_core::spa::store(&bridge, index, &s).map_err(|e| e.to_string())
}

/// Whether a table has a grid view.
#[tauri::command]
fn grid_supported(table: String) -> bool {
    kenon_core::grid::supported(&table)
}

/// A table as a grid for bulk editing (None when it has no grid view).
#[tauri::command]
async fn table_grid(
    state: State<'_, AppState>,
    table: String,
) -> Result<Option<kenon_core::grid::Grid>, String> {
    let bridge = state.need_bridge()?;
    if !kenon_core::grid::supported(&table) {
        return Ok(None);
    }
    kenon_core::grid::grid(&bridge, &table)
        .map(Some)
        .map_err(|e| e.to_string())
}

/// Saves edited grid cells; returns how many entries changed.
#[tauri::command]
fn set_grid_cells(
    state: State<AppState>,
    table: String,
    cells: Vec<kenon_core::grid::Cell>,
) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    kenon_core::grid::set_cells(&bridge, &table, &cells).map_err(|e| e.to_string())
}

#[tauri::command]
async fn move_users(
    state: State<'_, AppState>,
    id: u16,
) -> Result<kenon_core::battle::MoveUsers, String> {
    let bridge = state.need_bridge()?;
    kenon_core::battle::move_users(&bridge, id).map_err(|e| e.to_string())
}

/// Where the game tests an ability's id itself (in battle or outside it).
#[tauri::command]
fn ability_checks(id: u16) -> Vec<kenon_core::battle::CodeCheck> {
    kenon_core::battle::ability_checks(id)
}

#[tauri::command]
async fn ability_users(
    state: State<'_, AppState>,
    id: u8,
) -> Result<kenon_core::battle::AbilityUsers, String> {
    let bridge = state.need_bridge()?;
    kenon_core::battle::ability_users(&bridge, id).map_err(|e| e.to_string())
}

/// The move each TM/HM teaches (TM01..TM95, HM01..HM06).
#[tauri::command]
async fn machine_moves(state: State<'_, AppState>) -> Result<Vec<u16>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::battle::machine_moves(&bridge).map_err(|e| e.to_string())
}

/// Where an item is found or used (wild, trainers, trades, maps).
#[tauri::command]
async fn item_users(
    state: State<'_, AppState>,
    id: u16,
) -> Result<kenon_core::battle::ItemUsers, String> {
    let bridge = state.need_bridge()?;
    kenon_core::battle::item_users(&bridge, id).map_err(|e| e.to_string())
}

/// Names of the species and form records ("Deoxys (form 1)").
#[tauri::command]
async fn record_names(state: State<'_, AppState>) -> Result<Arc<Vec<String>>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::battle::record_names(&bridge).map_err(|e| e.to_string())
}

#[tauri::command]
async fn type_users(
    state: State<'_, AppState>,
    id: u8,
) -> Result<kenon_core::battle::TypeUsers, String> {
    let bridge = state.need_bridge()?;
    kenon_core::battle::type_users(&bridge, id).map_err(|e| e.to_string())
}

/// The ids a mod adds to a table (new moves, new items).
#[tauri::command]
async fn content_added(
    state: State<'_, AppState>,
    table: String,
) -> Result<kenon_core::content::Added, String> {
    let bridge = state.need_bridge()?;
    kenon_core::content::added(&bridge, &table).map_err(|e| e.to_string())
}

/// Adds an entry made from `from` with a new name; returns its id.
#[tauri::command]
fn content_add(
    state: State<AppState>,
    table: String,
    from: usize,
    name: String,
) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    let id = kenon_core::content::add(&bridge, &table, from, &name).map_err(|e| e.to_string())?;
    // The manifest's limit line is written on disk.
    reload_project(&state)?;
    Ok(id)
}

/// Removes an added entry; the later ones move down one id and the mod's
/// records follow.
#[tauri::command]
fn content_remove(
    state: State<AppState>,
    table: String,
    id: usize,
) -> Result<kenon_core::content::Removed, String> {
    let bridge = state.need_bridge()?;
    let removed = kenon_core::content::remove(&bridge, &table, id).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(removed)
}

/// The battle handler reference for a table (moves, items, abilities): the
/// copy the last eidos run left, else the bundled one.
#[tauri::command]
fn handler_params(
    state: State<AppState>,
    table: String,
) -> Result<kenon_core::handlers::Params, String> {
    let kind = kenon_core::handlers::kind_of(&table).ok_or("no battle handlers")?;
    let mut params = kenon_core::handlers::params(
        kind,
        kenon_core::data_root::eidos_handler_params().as_deref(),
    )?;
    if let Some(project) = state.need_bridge().ok().and_then(|b| b.project().cloned()) {
        params.add_types(&mod_type_words(&project));
    }
    Ok(params)
}

/// The mod's added types' words for `type=` keys.
fn mod_type_words(project: &kenon_core::project::Project) -> Vec<String> {
    kenon_core::type_pictures::key_names(project)
        .into_iter()
        .map(|(_, w)| w)
        .collect()
}

/// A move's animation: the one it plays and the one it would play with no
/// line of its own.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MoveAnim {
    anim: u16,
    default: u16,
    /// The mod ships a script for this move.
    own: bool,
}

#[tauri::command]
fn move_anim(state: State<AppState>, id: usize) -> Result<MoveAnim, String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    Ok(MoveAnim {
        anim: kenon_core::move_anims::current(project, id),
        default: kenon_core::move_anims::default_for(project, id),
        own: kenon_core::move_anims::has_own(project, id),
    })
}

/// Sets a move's animation (eidos move_anims.txt); None: its default.
#[tauri::command]
fn set_move_anim(state: State<AppState>, id: usize, anim: Option<u16>) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    kenon_core::move_anims::set(project, id, anim)?;
    bridge.invalidate();
    Ok(())
}

/// A move's animation script as text (None: an added move without one).
#[tauri::command]
fn move_script(state: State<AppState>, id: usize) -> Result<Option<String>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::move_anims::script_text(&bridge, id).map_err(|e| e.to_string())
}

/// Saves a move's animation script from text.
#[tauri::command]
fn set_move_script(state: State<AppState>, id: usize, text: String) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::move_anims::set_script(&bridge, id, &text).map_err(|e| e.to_string())
}

/// Gives a move its own script, copied from move `from`'s.
#[tauri::command]
fn copy_move_script(state: State<AppState>, id: usize, from: usize) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::move_anims::copy_script(&bridge, id, from).map_err(|e| e.to_string())
}

/// Drops the mod's script for a move.
#[tauri::command]
fn drop_move_script(state: State<AppState>, id: usize) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::move_anims::drop_script(&bridge, id).map_err(|e| e.to_string())
}

/// The added moves that have their own script (others can play them).
#[tauri::command]
fn own_move_scripts(state: State<AppState>) -> Result<Vec<usize>, String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    Ok(project
        .members(kenon_core::move_anims::SCRIPTS)
        .into_iter()
        .filter(|&m| m > kenon_core::move_anims::LAST as usize)
        .collect())
}

/// The first problem in a script's text: (line, message), None when it
/// assembles.
#[tauri::command]
fn check_effect_script(text: String) -> Option<(usize, String)> {
    let e = kenon_core::effscript::assemble(&text).err()?;
    let line = e
        .strip_prefix("line ")
        .and_then(|r| r.split_once(':'))
        .and_then(|(n, _)| n.parse().ok())
        .unwrap_or(1);
    let message = e.split_once(": ").map_or(e.clone(), |(_, m)| m.to_string());
    Some((line, message))
}

/// The running preview, so a new one replaces it.
static PREVIEW: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);

/// Starts eidos playing a move's animation from this mod, in a loop until
/// its window closes. When eidos refuses (exit code not 0), the
/// `preview-ended` event carries its message.
#[tauri::command]
fn preview_move(
    app: tauri::AppHandle,
    state: State<AppState>,
    id: usize,
    species: Option<(usize, usize)>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    let code = bridge
        .code()
        .map(|c| c.game_code().to_string())
        .unwrap_or_default();
    let profile = kenon_core::preview::profile_dir(&code).ok_or("no eidos user folder")?;
    let args = kenon_core::preview::move_args(&profile, &project.root, id, species);
    run_eidos(app, args)
}

/// Plays the mod in eidos on its own test save (eidos `--test-saves`):
/// the profile's save is copied once, then each test continues it; the
/// player's saves are never written. Refusals come as `preview-ended`.
#[tauri::command]
fn test_in_game(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    let code = bridge
        .code()
        .map(|c| c.game_code().to_string())
        .unwrap_or_default();
    let profile = kenon_core::preview::profile_dir(&code).ok_or("no eidos user folder")?;
    let folder = kenon_core::preview::test_folder(&project.root).ok_or("no eidos user folder")?;
    set_mod_language(&state, project)?;
    refresh_min_eidos(project)?;
    let args = kenon_core::preview::test_args(&profile, &project.root, &folder);
    run_eidos(app, args)
}

/// Starts eidos with `args`, replacing a running preview or test. When it
/// exits refusing (exit code not 0), the `preview-ended` event carries its
/// message.
fn run_eidos(app: tauri::AppHandle, args: Vec<std::ffi::OsString>) -> Result<(), String> {
    use std::io::BufRead;
    let (exe, _) = kenon_core::preview::program()
        .ok_or("eidos is not installed. Install it with the eidos launcher.")?;
    let mut slot = PREVIEW.lock().unwrap();
    if let Some(mut old) = slot.take() {
        let _ = old.kill();
        let _ = old.wait();
    }
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(&args).stderr(std::process::Stdio::piped());
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("eidos did not start: {e}"))?;
    let stderr = child.stderr.take();
    let pid = child.id();
    *slot = Some(child);
    drop(slot);
    std::thread::spawn(move || {
        // eidos's own message ("eidos: --preview-move: ..."), else its last line.
        let mut said = String::new();
        let mut last = String::new();
        if let Some(err) = stderr {
            for line in std::io::BufReader::new(err).lines().map_while(Result::ok) {
                if line.starts_with("eidos: --preview") || line.starts_with("eidos: --test-saves") {
                    said = line;
                } else if !line.trim().is_empty() {
                    last = line;
                }
            }
        }
        let status = {
            let mut slot = PREVIEW.lock().unwrap();
            match slot.as_mut().filter(|c| c.id() == pid) {
                Some(c) => {
                    let s = c.wait().ok();
                    *slot = None;
                    s
                }
                // Replaced by a newer preview, which killed it.
                None => return,
            }
        };
        if status.is_some_and(|s| !s.success()) {
            let msg = if said.is_empty() { last } else { said };
            let msg = preview_message(&msg);
            let _ = app.emit("preview-ended", msg);
        }
    });
    Ok(())
}

/// eidos's refusal without the program and flag names: "eidos:
/// --preview-move: no move 9999 (...)" becomes "No move 9999 (...)".
fn preview_message(line: &str) -> String {
    let rest = line.trim().trim_start_matches("eidos:").trim_start();
    if rest.starts_with("--preview-move needs a save") {
        return "Start a game in eidos once first. The preview needs a save.".into();
    }
    let rest = ["--preview-move", "--preview-species", "--test-saves"]
        .iter()
        .find_map(|f| rest.strip_prefix(f))
        .map_or(rest, |r| r.trim_start_matches(':').trim_start());
    let mut c = rest.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// Battle code eidos adds that no game entry has: (name, kenon number).
#[tauri::command]
fn named_bases() -> Vec<(&'static str, usize)> {
    kenon_core::handlers::NAMED_BASES.to_vec()
}

/// The mod's handler rows for a table: id to {base, keys}.
#[tauri::command]
fn handler_rows(
    state: State<AppState>,
    table: String,
) -> Result<std::collections::BTreeMap<usize, kenon_core::handlers::Row>, String> {
    let bridge = state.need_bridge()?;
    let kind = kenon_core::handlers::kind_of(&table).ok_or("no battle handlers")?;
    Ok(bridge
        .project()
        .map(|p| kenon_core::handlers::rows(p, kind))
        .unwrap_or_default())
}

/// Sets what an entry does in battle (None: its own code).
#[tauri::command]
fn set_handler(
    state: State<AppState>,
    table: String,
    id: usize,
    row: Option<kenon_core::handlers::Row>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let kind = kenon_core::handlers::kind_of(&table).ok_or("no battle handlers")?;
    let project = bridge.project().ok_or("no project is open")?;
    let mut params = kenon_core::handlers::params(
        kind,
        kenon_core::data_root::eidos_handler_params().as_deref(),
    )?;
    params.add_types(&mod_type_words(project));
    kenon_core::handlers::set(project, kind, &params, id, row)
}

/// Names for the text banks of an archive kenon can name (system banks by
/// use, story banks by the places that speak from them).
#[tauri::command]
async fn text_bank_names(
    state: State<'_, AppState>,
    archive: u16,
) -> Result<std::collections::HashMap<usize, String>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::world::text_bank_names(&bridge, archive).map_err(|e| e.to_string())
}

/// A trainer's per-member numbers (IVs, EVs, nature, shiny, friendship,
/// ability), six slots; the game's own where the mod sets none.
#[tauri::command]
fn trainer_mons(
    state: State<AppState>,
    trainer: usize,
) -> Result<Vec<kenon_core::trainer_mons::Extras>, String> {
    let bridge = state.need_bridge()?;
    Ok(bridge
        .project()
        .map(|p| kenon_core::trainer_mons::of(p, trainer))
        .unwrap_or_else(|| vec![Default::default(); 6]))
}

/// Sets a trainer's per-member numbers, slot by slot.
#[tauri::command]
fn set_trainer_mons(
    state: State<AppState>,
    trainer: usize,
    members: Vec<kenon_core::trainer_mons::Extras>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    kenon_core::trainer_mons::set(project, trainer, &members).map_err(|e| e.to_string())
}

/// The shop lists (the mod's over the game's) and the game's own.
#[tauri::command]
async fn shops(
    state: State<'_, AppState>,
) -> Result<(kenon_core::shops::Shops, kenon_core::shops::Shops), String> {
    let bridge = state.need_bridge()?;
    let now = kenon_core::shops::current(&bridge).map_err(|e| e.to_string())?;
    let game = kenon_core::shops::game(&bridge).map_err(|e| e.to_string())?;
    Ok((now, game))
}

/// Every item's shop price (item data price x 10), by item id.
#[tauri::command]
async fn item_prices(state: State<'_, AppState>) -> Result<Vec<u32>, String> {
    let bridge = state.need_bridge()?;
    let count = kenon_core::tables::entries(&bridge, "items")
        .map_err(|e| e.to_string())?
        .len();
    Ok((0..count)
        .map(|index| {
            bridge
                .member(kenon_core::bridge::Member { arc: 24, index })
                .ok()
                .and_then(|b| {
                    b.get(0..2)
                        .map(|p| u32::from(u16::from_le_bytes([p[0], p[1]])) * 10)
                })
                .unwrap_or(0)
        })
        .collect())
}

/// Writes every shop list (eidos shops.txt).
#[tauri::command]
fn set_shops(state: State<AppState>, shops: kenon_core::shops::Shops) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::shops::set(&bridge, &shops).map_err(|e| e.to_string())
}

/// The 101 TMs and HMs: label, item, move (the mod's over the game's).
#[tauri::command]
async fn machines(
    state: State<'_, AppState>,
) -> Result<Vec<kenon_core::machines::Machine>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::machines::list(&bridge).map_err(|e| e.to_string())
}

/// Sets which move machine `index` teaches (and its item description).
#[tauri::command]
fn set_machine(state: State<AppState>, index: usize, mv: u16) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::machines::set(&bridge, index, mv).map_err(|e| e.to_string())
}

/// Adds the next new TM (TM96 on), teaching `mv`; returns its index.
#[tauri::command]
fn add_machine(state: State<AppState>, mv: u16) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    let i = kenon_core::machines::add(&bridge, mv).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(i)
}

/// Removes the last new TM; returns its label.
#[tauri::command]
fn remove_last_machine(state: State<AppState>) -> Result<String, String> {
    let bridge = state.need_bridge()?;
    let l = kenon_core::machines::remove_last(&bridge).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(l)
}

/// An icon's palettes: (male, female), 0 to 15 each.
#[tauri::command]
fn icon_palette(state: State<AppState>, icon: usize) -> Result<(u8, u8), String> {
    let bridge = state.need_bridge()?;
    let b = kenon_core::species_add::icon_palette(&bridge, icon);
    Ok((b & 0xF, b >> 4))
}

/// Sets an icon's palettes (eidos icon_palette.txt).
#[tauri::command]
fn set_icon_palette(
    state: State<AppState>,
    icon: usize,
    male: u8,
    female: u8,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::species_add::set_icon_palette(&bridge, icon, male, female)
        .map_err(|e| e.to_string())
}

/// Each member of a 3D archive named by its first texture ("" when none).
#[tauri::command]
async fn texture_set_names(
    state: State<'_, AppState>,
    archive: u16,
) -> Result<Vec<String>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::catalog::texture_set_names(&bridge, archive)
        .map(|n| n.as_ref().clone())
        .map_err(|e| e.to_string())
}

/// Where each song plays: sequence number to places.
#[tauri::command]
async fn music_places(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<u16, String>, String> {
    let bridge = state.need_bridge()?;
    kenon_core::world::music_places(&bridge).map_err(|e| e.to_string())
}

/// Sets single text lines of a/0/0/2: [bank, index, text] each.
#[tauri::command]
fn set_text_lines(
    state: State<AppState>,
    lines: Vec<(usize, usize, String)>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    kenon_core::catalog::set_text_lines(&bridge, 2, &lines).map_err(|e| e.to_string())
}

/// The species ids the mod adds (715 and up), forms not included.
#[tauri::command]
fn species_added(state: State<AppState>) -> Result<Vec<usize>, String> {
    let bridge = state.need_bridge()?;
    let l = kenon_core::species_add::layout(&bridge);
    Ok((kenon_core::species_add::FIRST_NEW..=l.last).collect())
}

/// The forms of the Pokemon the mod adds: species to its form slots
/// (form 1 first).
#[tauri::command]
fn species_forms(
    state: State<AppState>,
) -> Result<std::collections::BTreeMap<usize, Vec<usize>>, String> {
    let bridge = state.need_bridge()?;
    Ok(kenon_core::species_add::layout(&bridge).forms)
}

/// Adds a form to a Pokemon the mod adds, made from slot `from`; returns
/// its form number.
#[tauri::command]
fn species_add_form(state: State<AppState>, species: usize, from: usize) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    let f = kenon_core::species_add::add_form(&bridge, species, from).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(f)
}

/// Removes a form of a Pokemon the mod adds.
#[tauri::command]
fn species_remove_form(
    state: State<AppState>,
    species: usize,
    form: usize,
) -> Result<kenon_core::content::Removed, String> {
    let bridge = state.need_bridge()?;
    let r =
        kenon_core::species_add::remove_form(&bridge, species, form).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(r)
}

/// The cry a Pokemon or form plays: its wave archive slot, the default
/// one, and the species whose cry that slot is (0: a cry the mod added).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CryChoice {
    slot: u32,
    default: u32,
    /// The wave archive's name in the sound archive.
    name: String,
}

#[tauri::command]
fn species_cry(state: State<AppState>, species: usize, form: usize) -> Result<CryChoice, String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    let key = kenon_core::cries::Key { species, form };
    let slot = kenon_core::cries::current(project, key);
    let name = bridge
        .sdat()
        .and_then(|s| {
            s.name(kenon_core::sdat::Kind::WaveArc, slot)
                .map(str::to_string)
        })
        .unwrap_or_default();
    Ok(CryChoice {
        slot,
        default: kenon_core::cries::default_for(project, key),
        name,
    })
}

/// Sets the cry slot of a Pokemon or form (None: its default).
#[tauri::command]
fn set_species_cry(
    state: State<AppState>,
    species: usize,
    form: usize,
    slot: Option<u32>,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    kenon_core::cries::set(project, kenon_core::cries::Key { species, form }, slot)?;
    bridge.invalidate();
    Ok(())
}

/// A Pokemon's Unova Pokedex number (None: national only), the game's,
/// and the next free number.
#[tauri::command]
fn regional_dex(
    state: State<AppState>,
    species: usize,
) -> Result<kenon_core::regional_dex::Info, String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    Ok(kenon_core::regional_dex::info(&bridge, project, species))
}

/// Sets a Pokemon's place in the Unova Pokedex; returns what it shows now.
#[tauri::command]
fn set_regional_dex(
    state: State<AppState>,
    species: usize,
    place: kenon_core::regional_dex::Place,
) -> Result<kenon_core::regional_dex::Info, String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    kenon_core::regional_dex::set(&bridge, project, species, place)?;
    bridge.invalidate();
    Ok(kenon_core::regional_dex::info(&bridge, project, species))
}

/// A Pokemon's or form's Pokedex search shape and letter group (None: not
/// set), and the game's.
#[derive(Serialize)]
struct DexSearch {
    current: kenon_core::dex_search::Search,
    game: kenon_core::dex_search::Search,
    shapes: &'static [&'static str],
}

#[tauri::command]
fn dex_search(state: State<AppState>, species: usize, form: usize) -> Result<DexSearch, String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    let key = kenon_core::cries::Key { species, form };
    Ok(DexSearch {
        current: kenon_core::dex_search::current(&bridge, project, key),
        game: if form == 0 {
            kenon_core::dex_search::game(&bridge, species)
        } else {
            Default::default()
        },
        shapes: &kenon_core::dex_search::SHAPES_NAMED,
    })
}

#[tauri::command]
fn set_dex_search(
    state: State<AppState>,
    species: usize,
    form: usize,
    search: kenon_core::dex_search::Search,
) -> Result<(), String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?;
    kenon_core::dex_search::set(
        &bridge,
        project,
        kenon_core::cries::Key { species, form },
        search,
    )?;
    bridge.invalidate();
    Ok(())
}

/// Gives a Pokemon or form a brand-new cry from a WAV file: a wave archive
/// of its own, appended to the sound archive.
#[tauri::command]
async fn import_new_cry(
    state: State<'_, AppState>,
    species: usize,
    form: usize,
    name: String,
    path: PathBuf,
) -> Result<kenon_core::audio_import::AudioSummary, String> {
    let bridge = state.need_bridge()?;
    let project = bridge.project().ok_or("no project is open")?.clone();
    let wav = std::fs::read(&path).map_err(|e| format!("could not read the file: {e}"))?;
    let (slot, summary) =
        kenon_core::sound_add::add_cry(&bridge, &name, &wav).map_err(|e| e.to_string())?;
    kenon_core::cries::set(
        &project,
        kenon_core::cries::Key { species, form },
        Some(slot),
    )?;
    bridge.invalidate();
    Ok(summary)
}

/// Adds a Pokemon made from `from`; returns its id.
#[tauri::command]
fn species_add(state: State<AppState>, from: usize, name: String) -> Result<usize, String> {
    let bridge = state.need_bridge()?;
    let id = kenon_core::species_add::add(&bridge, from, &name).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(id)
}

/// Removes a Pokemon the mod adds; later ones move down one id.
#[tauri::command]
fn species_remove(
    state: State<AppState>,
    id: usize,
) -> Result<kenon_core::content::Removed, String> {
    let bridge = state.need_bridge()?;
    let removed = kenon_core::species_add::remove(&bridge, id).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(removed)
}

/// Gives an added item its own icon from a PNG file.
#[tauri::command]
async fn content_item_icon(
    state: State<'_, AppState>,
    item: usize,
    path: PathBuf,
) -> Result<Vec<u32>, String> {
    let bridge = state.need_bridge()?;
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read the file: {e}"))?;
    kenon_core::content::set_item_icon(&bridge, item, &bytes).map_err(|e| e.to_string())
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
    // The mod's added moves too (kenon_core::content).
    let moves = kenon_core::content::kind("moves").ok_or("no moves kind")?;
    let count = kenon_core::content::count(&bridge, moves).map_err(|e| e.to_string())?;
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

// The mod code editors (morphe, crates/eidos-mod/src/modcode.rs). Each command that changes the
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
    let mut found = modcode::check_settings(&settings, hooks.lines().count());
    found.extend(modcode::check_table_files(&root).map_err(|e| e.to_string())?);
    found.extend(modcode::check_session_use(&root, &settings));
    found.extend(modcode::check_icon_file(&root, &settings));
    Ok(found)
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

/// The mod's tables are the ones a rule may read; with no project open the
/// table names are not checked.
#[tauri::command]
fn mod_check_rule(
    state: State<AppState>,
    text: String,
    event: Option<String>,
) -> Vec<modcode::Diagnostic> {
    let root = mod_root(&state).ok();
    let tables = root.as_ref().and_then(|r| modcode::table_set(r).ok());
    let settings = root
        .as_ref()
        .and_then(|r| kenon_core::modopts::settings_ctx(r).ok());
    let ctx = modcode::RuleContext {
        tables: tables.as_ref(),
        settings: settings.as_ref(),
        root: root.as_deref(),
    };
    modcode::check_rule_ctx(&text, event.as_deref(), ctx)
}

#[tauri::command]
fn mod_read_options(state: State<AppState>) -> Result<kenon_core::modopts::OptionsFile, String> {
    let root = mod_root(&state)?;
    kenon_core::modopts::read_options_file(&root).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_save_options(
    state: State<AppState>,
    options: Vec<kenon_core::modopts::OptionDecl>,
) -> Result<kenon_core::modopts::OptionsFile, String> {
    let root = mod_root(&state)?;
    kenon_core::modopts::write_options_file(&root, &options).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_check_options(options: Vec<kenon_core::modopts::OptionDecl>) -> Vec<modcode::Diagnostic> {
    kenon_core::modopts::check_options(&options)
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

#[tauri::command]
fn mod_list_tables(state: State<AppState>) -> Result<Vec<modcode::TableEntry>, String> {
    let root = mod_root(&state)?;
    modcode::list_tables(&root).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_read_table(state: State<AppState>, name: String) -> Result<String, String> {
    let root = mod_root(&state)?;
    modcode::read_table(&root, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_check_table(text: String) -> Vec<modcode::Diagnostic> {
    modcode::check_table(&text)
}

#[tauri::command]
fn mod_save_table(
    state: State<AppState>,
    name: String,
    text: String,
) -> Result<Vec<modcode::Diagnostic>, String> {
    let root = mod_root(&state)?;
    let found = modcode::write_table(&root, &name, &text).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(found)
}

#[tauri::command]
fn mod_new_table(state: State<AppState>, name: String) -> Result<modcode::TableEntry, String> {
    let root = mod_root(&state)?;
    let made = modcode::new_table(&root, &name).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(made)
}

/// The renamed table and how many rule reads were changed with it.
#[tauri::command]
fn mod_rename_table(
    state: State<AppState>,
    from: String,
    to: String,
) -> Result<(modcode::TableEntry, usize), String> {
    let root = mod_root(&state)?;
    let done = modcode::rename_table(&root, &from, &to).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(done)
}

#[tauri::command]
fn mod_delete_table(state: State<AppState>, name: String) -> Result<(), String> {
    let root = mod_root(&state)?;
    modcode::delete_table(&root, &name).map_err(|e| e.to_string())?;
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

// Code mods (core/src/codemod.rs): the C template, the Zig compiler and the
// build.

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ZigStatus {
    /// The compiler in use, if one is found.
    path: Option<PathBuf>,
    /// `own` (chosen by the user), `downloaded` or `path`.
    source: Option<&'static str>,
    version: Option<String>,
    /// The release kenon downloads, and its size in MB (None when there is
    /// no download for this system).
    pinned: &'static str,
    download_mb: Option<u64>,
    /// The Zig the user chose, even when it is not found.
    chosen: Option<PathBuf>,
}

fn app_data(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn zig_status_of(state: &AppState, app: &tauri::AppHandle) -> Result<ZigStatus, String> {
    let chosen = state.settings.lock().unwrap().zig_path.clone();
    let found = codemod::zig_path(chosen.as_deref(), &app_data(app)?);
    let release = codemod::zig_release(std::env::consts::OS, std::env::consts::ARCH);
    Ok(ZigStatus {
        version: found.as_ref().and_then(|(p, _)| codemod::zig_version(p)),
        source: found.as_ref().map(|(_, s)| *s),
        path: found.map(|(p, _)| p),
        pinned: codemod::ZIG_VERSION,
        download_mb: release.map(|r| r.size.div_ceil(1_000_000)),
        chosen,
    })
}

#[tauri::command]
async fn zig_status(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ZigStatus, String> {
    zig_status_of(&state, &app)
}

/// Uses the Zig at `path` (or goes back to the downloaded one with `None`).
#[tauri::command]
async fn set_zig_path(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    path: Option<PathBuf>,
) -> Result<ZigStatus, String> {
    if let Some(path) = &path {
        let exe = if path.is_dir() {
            path.join(if cfg!(windows) { "zig.exe" } else { "zig" })
        } else {
            path.clone()
        };
        if codemod::zig_version(&exe).is_none() {
            return Err("this file does not run as Zig".to_string());
        }
    }
    {
        let mut settings = state.settings.lock().unwrap();
        settings.zig_path = path;
        state.save_settings(&settings)?;
    }
    zig_status_of(&state, &app)
}

/// Downloads the pinned Zig. Progress goes to the `zig-progress` event.
#[tauri::command]
async fn fetch_zig(state: State<'_, AppState>, app: tauri::AppHandle) -> Result<ZigStatus, String> {
    let data = app_data(&app)?;
    let events = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        codemod::fetch_zig(&data, &mut |p| {
            let _ = events.emit("zig-progress", p);
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    // The downloaded one is used from now on.
    {
        let mut settings = state.settings.lock().unwrap();
        if settings.zig_path.is_some() {
            settings.zig_path = None;
            state.save_settings(&settings)?;
        }
    }
    zig_status_of(&state, &app)
}

fn code_project(state: &AppState) -> Result<Arc<Project>, String> {
    state
        .open
        .read()
        .unwrap()
        .project
        .clone()
        .ok_or_else(|| "open a project first".to_string())
}

/// Writes the starting files of a code mod and sets the manifest.
#[tauri::command]
fn new_code_mod(state: State<AppState>) -> Result<Vec<String>, String> {
    let project = code_project(&state)?;
    let written = codemod::new_code_mod(&project).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(written)
}

#[tauri::command]
async fn build_code_mod(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<codemod::BuildResult, String> {
    let project = code_project(&state)?;
    let chosen = state.settings.lock().unwrap().zig_path.clone();
    let data = app_data(&app)?;
    let (zig, _) = codemod::zig_path(chosen.as_deref(), &data)
        .ok_or_else(|| "kenon needs the Zig compiler to build code".to_string())?;
    let cache = data.join("zig-cache");
    tauri::async_runtime::spawn_blocking(move || {
        codemod::build_code_mod(&project, &zig, Some(&cache))
    })
    .await
    .map_err(|e| e.to_string())
}

/// Copies the mod C library into the project (morphe's modsdk/v7/libc).
#[tauri::command]
fn mod_code_add_libc(state: State<AppState>) -> Result<Vec<String>, String> {
    let written = codemod::add_libc(&mod_root(&state)?).map_err(|e| e.to_string())?;
    reload_project(&state)?;
    Ok(written)
}

#[tauri::command]
fn mod_code_files(state: State<AppState>) -> Result<Vec<codemod::SourceFile>, String> {
    Ok(codemod::list_sources(&mod_root(&state)?))
}

#[tauri::command]
fn mod_code_read(state: State<AppState>, path: String) -> Result<String, String> {
    codemod::read_source(&mod_root(&state)?, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_code_write(state: State<AppState>, path: String, text: String) -> Result<(), String> {
    codemod::write_source(&mod_root(&state)?, &path, &text).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_code_new(state: State<AppState>, path: String) -> Result<(), String> {
    codemod::new_source(&mod_root(&state)?, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_code_rename(state: State<AppState>, from: String, to: String) -> Result<(), String> {
    codemod::rename_source(&mod_root(&state)?, &from, &to).map_err(|e| e.to_string())
}

#[tauri::command]
fn mod_code_delete(state: State<AppState>, path: String) -> Result<(), String> {
    codemod::delete_source(&mod_root(&state)?, &path).map_err(|e| e.to_string())
}

/// The names the C editor completes, from the mod's own header.
#[tauri::command]
fn mod_code_api(state: State<AppState>) -> Result<codemod::CodeApi, String> {
    Ok(codemod::code_api(&mod_root(&state)?))
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
                references: Mutex::new(std::collections::HashMap::new()),
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
            recent_projects,
            forget_recent,
            close_project,
            export_bw,
            languages,
            set_reference_rom,
            clear_reference_rom,
            reference_text,
            mod_lang_lines,
            set_mod_lang_lines,
            translation_rows,
            set_translation,
            official_names,
            import_text,
            mod_min_eidos,
            import_bw,
            import_patch,
            archives,
            export_asset,
            export_member,
            export_text,
            table_archive,
            members,
            changed_members,
            sounds,
            import_cry,
            import_sound,
            revert_cry,
            revert_sound,
            audio_changes,
            add_sound,
            add_character,
            added_characters,
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
            content_added,
            battle_coded,
            battle_coded_all,
            type_chart,
            set_type_chart,
            add_type,
            type_look,
            move_kinds,
            item_ball,
            set_item_ball,
            set_move_kinds,
            item_heal,
            set_item_heal,
            add_type_plate,
            add_type_drive,
            game_form_count,
            add_arceus_form,
            set_type_plate,
            set_type_drive,
            remake_type_pictures,
            set_type_colors,
            import_type_icon,
            remove_type,
            type_count,
            table_grid,
            particle_list,
            particle_fields,
            particle_file,
            set_particle_file,
            add_particle_file,
            import_particle_texture,
            grid_supported,
            set_grid_cells,
            move_users,
            ability_users,
            ability_checks,
            type_users,
            item_users,
            machine_moves,
            record_names,
            content_add,
            content_remove,
            handler_params,
            text_bank_names,
            machines,
            set_machine,
            add_machine,
            remove_last_machine,
            shops,
            set_shops,
            species_forms,
            regional_dex,
            set_regional_dex,
            dex_search,
            set_dex_search,
            species_add_form,
            species_remove_form,
            species_cry,
            set_species_cry,
            import_new_cry,
            move_anim,
            move_script,
            set_move_script,
            copy_move_script,
            drop_move_script,
            own_move_scripts,
            check_effect_script,
            set_move_anim,
            preview_move,
            test_in_game,
            item_prices,
            icon_palette,
            set_icon_palette,
            trainer_mons,
            set_trainer_mons,
            texture_set_names,
            music_places,
            handler_rows,
            named_bases,
            set_handler,
            content_item_icon,
            species_added,
            set_text_lines,
            species_add,
            species_remove,
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
            mod_read_options,
            mod_save_options,
            mod_check_options,
            mod_save_rule,
            mod_new_rule,
            mod_rename_rule,
            mod_delete_rule,
            mod_list_tables,
            mod_read_table,
            mod_check_table,
            mod_save_table,
            mod_new_table,
            mod_rename_table,
            mod_delete_table,
            mod_get_paths,
            mod_set_paths,
            mod_conflicts,
            zig_status,
            set_zig_path,
            fetch_zig,
            new_code_mod,
            build_code_mod,
            mod_code_add_libc,
            mod_code_files,
            mod_code_read,
            mod_code_write,
            mod_code_new,
            mod_code_rename,
            mod_code_delete,
            mod_code_api,
        ])
        .run(tauri::generate_context!())
        .expect("error while running kenon");
}
