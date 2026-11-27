//! An added type's plate and drive (eidos docs/game-tables.md, types.txt
//! `plate <item>` and `drive <item> <Genesect form>`): Arceus with
//! Multitype holding the plate takes the type and Judgment follows; Techno
//! Blast takes the drive's type. A new plate is a copy of the Flame Plate
//! that boosts the type (an item_handlers.txt line, `type=<word>`); a new
//! drive is a copy of the Douse Drive.

use crate::bridge::{Bridge, Error};
use crate::type_pictures::{self, TypeLook};

/// Flame Plate, the plate a new one copies (Fire: its words name the type).
pub const PLATE_FROM: usize = 298;
/// Douse Drive, the drive a new one copies (Water).
pub const DRIVE_FROM: usize = 116;
/// The first Genesect form a drive may give (1 to 4 are the game's drives).
pub const FIRST_DRIVE_FORM: usize = 5;

/// Item text banks: name, plural, description (a/0/0/2).
const PLURAL: usize = 280;
const DESCRIPTION: usize = 53;

fn update(bridge: &Bridge, t: usize, f: impl FnOnce(&mut TypeLook)) -> Result<(), Error> {
    let project = bridge.project().ok_or_else(|| Error::BadPath("no project is open".into()))?;
    let mut rows = type_pictures::all(project);
    let look = rows.get_mut(&t).ok_or_else(|| Error::Missing(format!("type {t} has no pictures yet")))?;
    f(look);
    type_pictures::write_rows(project, &rows)?;
    bridge.invalidate();
    Ok(())
}

/// After an added item is removed: a plate or drive that was it goes, and
/// later items move down one. Returns whether anything changed.
pub fn item_removed(project: &crate::project::Project, id: usize) -> Result<bool, Error> {
    let shift = |v: usize| -> Option<usize> {
        match v.cmp(&id) {
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(v - 1),
            std::cmp::Ordering::Less => Some(v),
        }
    };
    let mut rows = type_pictures::all(project);
    let mut changed = false;
    for l in rows.values_mut() {
        let plate = l.plate.and_then(shift);
        let drive = l.drive.and_then(|(i, f)| shift(i).map(|i| (i, f)));
        if (plate, drive) != (l.plate, l.drive) {
            (l.plate, l.drive, changed) = (plate, drive, true);
        }
    }
    if changed {
        type_pictures::write_rows(project, &rows)?;
    }
    Ok(changed)
}

/// Sets or clears the type's plate.
pub fn set_plate(bridge: &Bridge, t: usize, item: Option<usize>) -> Result<(), Error> {
    update(bridge, t, |l| l.plate = item)
}

/// Sets or clears the type's drive and the Genesect form it gives.
pub fn set_drive(bridge: &Bridge, t: usize, drive: Option<(usize, usize)>) -> Result<(), Error> {
    if let Some((_, form)) = drive {
        if form < FIRST_DRIVE_FORM {
            return Err(Error::BadPath(format!("a new drive's Genesect form is {FIRST_DRIVE_FORM} or more")));
        }
    }
    update(bridge, t, |l| l.drive = drive)
}

/// The type's name and its word in eidos files.
fn names(bridge: &Bridge, t: usize) -> Result<(String, String), Error> {
    let project = bridge.project().ok_or_else(|| Error::BadPath("no project is open".into()))?;
    let name = crate::tables::enum_values(bridge, "type")?
        .get(t)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("no type {t}")))?;
    let word = type_pictures::all(project)
        .get(&t)
        .and_then(|l| l.name.clone())
        .ok_or_else(|| Error::Missing(format!("type {t} has no word in types.txt yet")))?;
    Ok((name, word))
}

/// The copy's texts: the source's with its type's name swapped for this one.
fn retext(bridge: &Bridge, from: usize, to: usize, from_type: &str, name: &str, item_name: &str) -> Result<(), Error> {
    let line = |bank: usize| -> Result<String, Error> {
        Ok(crate::catalog::text_bank(bridge, 2, bank)?.get(from).cloned().unwrap_or_default())
    };
    let plural = format!("{item_name}s");
    let description = line(DESCRIPTION)?.replace(from_type, name);
    crate::catalog::set_text_lines(bridge, 2, &[(PLURAL, to, plural), (DESCRIPTION, to, description)])
}

/// Adds "<Type> Plate", a copy of the Flame Plate that boosts the type's
/// moves, and makes it the type's plate. Returns its item id.
pub fn add_plate(bridge: &Bridge, t: usize, params: &crate::handlers::Params) -> Result<usize, Error> {
    let (name, word) = names(bridge, t)?;
    let item_name = format!("{name} Plate");
    let id = crate::content::add(bridge, "items", PLATE_FROM, &item_name)?;
    retext(bridge, PLATE_FROM, id, "Fire", &name, &item_name)?;
    let project = bridge.project().ok_or_else(|| Error::BadPath("no project is open".into()))?;
    let mut params = params.clone();
    params.add_types(std::slice::from_ref(&word));
    let part = crate::handlers::Part {
        base: PLATE_FROM,
        keys: [("type".to_string(), word)].into_iter().collect(),
    };
    crate::handlers::set(project, "item", &params, id, Some(vec![part])).map_err(Error::BadPath)?;
    set_plate(bridge, t, Some(id))?;
    Ok(id)
}

/// Adds "<Type> Drive", a copy of the Douse Drive, and makes it the type's
/// drive giving Genesect `form`. Returns its item id.
pub fn add_drive(bridge: &Bridge, t: usize, form: usize) -> Result<usize, Error> {
    if form < FIRST_DRIVE_FORM {
        return Err(Error::BadPath(format!("a new drive's Genesect form is {FIRST_DRIVE_FORM} or more")));
    }
    let (name, _) = names(bridge, t)?;
    let item_name = format!("{name} Drive");
    let id = crate::content::add(bridge, "items", DRIVE_FROM, &item_name)?;
    retext(bridge, DRIVE_FROM, id, "Water", &name, &item_name)?;
    set_drive(bridge, t, Some((id, form)))?;
    Ok(id)
}
