//! New moves and items against the user's ROM. Set `KENON_ROM` to run them;
//! without it they pass without doing anything.

use std::sync::Arc;

use kenon_core::bridge::{Bridge, Member};
use kenon_core::project::Project;
use kenon_core::rom::Rom;
use kenon_core::{catalog, content, tables};

fn bridge() -> Option<(Bridge, tempfile::TempDir)> {
    let path = std::env::var_os("KENON_ROM")?;
    let dir = tempfile::tempdir().unwrap();
    let project = Arc::new(Project::create(&dir.path().join("m"), "Test", None).unwrap());
    let bridge = Bridge::new(Rom::open(path.as_ref()).unwrap()).with_project(Some(project));
    Some((bridge, dir))
}

fn reopen(bridge: Bridge, dir: &tempfile::TempDir) -> Bridge {
    let project = Arc::new(Project::open(&dir.path().join("m")).unwrap());
    bridge.with_project(Some(project))
}

#[test]
fn a_move_is_added_with_its_text_and_removed() {
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let root = dir.path().join("m");
    let k = content::kind("moves").unwrap();
    assert_eq!(content::count(&bridge, k).unwrap(), 560);

    // Thunderbolt is move 85.
    let id = content::add(&bridge, "moves", 85, "Storm Bolt").unwrap();
    assert_eq!(id, 560);
    let bridge = reopen(bridge, &dir);

    // The record is Thunderbolt's.
    let rec = |i| bridge.member(Member { arc: 21, index: i }).unwrap();
    assert_eq!(rec(560), rec(85));

    // Five text lines: name, capitals, description, three battle lines.
    let bank = |b| catalog::text_bank(&bridge, 2, b).unwrap();
    assert_eq!(bank(203)[560], "Storm Bolt");
    assert_eq!(bank(286)[560], "STORM BOLT");
    assert_eq!(bank(202)[560], bank(202)[85]);
    let thunderbolt = bank(203)[85].clone();
    for n in 0..3 {
        let line = &bank(13)[560 * 3 + n];
        assert!(line.contains("Storm Bolt"), "{line}");
        assert_eq!(
            *line,
            bank(13)[85 * 3 + n].replace(&thunderbolt, "Storm Bolt")
        );
    }
    // The game's own lines are untouched.
    assert_eq!(bank(203).len(), 561);
    assert_eq!(bank(13).len(), 561 * 3);

    // Line files, not whole banks; the limit is declared.
    assert!(root.join("a/0/0/2/203.msg").is_file());
    assert!(!root.join("a/0/0/2/203").exists());
    let manifest = std::fs::read_to_string(root.join("eidos_mod.txt")).unwrap();
    assert!(manifest.contains("limit.moves = 560"), "{manifest}");

    // The table lists it by name.
    let entries = tables::entries(&bridge, "moves").unwrap();
    assert_eq!(entries.len(), 561);
    assert_eq!(entries[560].name, "Storm Bolt");

    // A game move cannot be removed; removing every added one leaves
    // nothing behind.
    let second = content::add(&bridge, "moves", 1, "Tap").unwrap();
    assert_eq!(second, 561);
    let bridge = reopen(bridge, &dir);
    assert!(content::remove(&bridge, "moves", 85).is_err());
    assert!(content::remove(&bridge, "moves", 562).is_err());
    content::remove(&bridge, "moves", 560).unwrap();
    let bridge = reopen(bridge, &dir);
    // Tap moved down to 560.
    assert_eq!(catalog::text_bank(&bridge, 2, 203).unwrap()[560], "Tap");
    content::remove(&bridge, "moves", 560).unwrap();
    let bridge = reopen(bridge, &dir);
    assert_eq!(content::count(&bridge, k).unwrap(), 560);
    assert!(!root.join("a/0/0/2/203.msg").exists());
    assert!(!root.join("a/0/2/1/560").exists());
    let manifest = std::fs::read_to_string(root.join("eidos_mod.txt")).unwrap();
    assert!(!manifest.contains("limit.moves"), "{manifest}");
}

#[test]
fn an_item_is_added_with_its_icon_row() {
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let root = dir.path().join("m");
    // Potion is item 17.
    let id = content::add(&bridge, "items", 17, "Mega Potion").unwrap();
    assert_eq!(id, 627);
    let bridge = reopen(bridge, &dir);
    let k = content::kind("items").unwrap();
    let potion = content::icon_row(&bridge, k, 17).unwrap();
    assert_eq!(content::icon_row(&bridge, k, 627).unwrap(), potion);
    let rows = std::fs::read_to_string(root.join("eidos/tables/item_icons.txt")).unwrap();
    assert!(
        rows.contains(&format!("627 = {} {}", potion[0], potion[1])),
        "{rows}"
    );
    let bank = |b| catalog::text_bank(&bridge, 2, b).unwrap();
    assert_eq!(bank(54)[627], "Mega Potion");
    assert_eq!(bank(280)[627], "Mega Potion");
    assert_eq!(bank(53)[627], bank(53)[17]);
    let manifest = std::fs::read_to_string(root.join("eidos_mod.txt")).unwrap();
    assert!(manifest.contains("limit.items = 627"), "{manifest}");

    // The battle bag's table (a/2/3/2) gets a byte for it, Potion's (HP/PP).
    let pockets = bridge.member(Member { arc: 232, index: 0 }).unwrap();
    assert_eq!(pockets.len(), 628);
    assert_eq!(pockets[627], 4);
    assert_eq!(pockets[17], 4);
    // Removing it gives the game's own table back.
    content::remove(&bridge, "items", 627).unwrap();
    assert!(
        !root.join("a/2/3/2").exists()
            || std::fs::read_dir(root.join("a/2/3/2"))
                .unwrap()
                .next()
                .is_none()
    );
    assert_eq!(
        bridge.member(Member { arc: 232, index: 0 }).unwrap().len(),
        627
    );
}

#[test]
fn a_line_file_is_not_a_member() {
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let root = dir.path().join("m");
    content::add(&bridge, "moves", 85, "Storm Bolt").unwrap();
    let bridge = reopen(bridge, &dir);
    // Saving bank 203 whole from the text browser keeps the new line in the
    // line file and leaves the bank member alone when nothing else changed.
    let strings = catalog::text_bank(&bridge, 2, 203).unwrap();
    catalog::save_text_bank(&bridge, 2, 203, &strings).unwrap();
    assert!(root.join("a/0/0/2/203.msg").is_file());
    assert!(!root.join("a/0/0/2/203").exists());
    // An edit of a game line goes into the member, the new line stays a line.
    let mut edited = strings.clone();
    edited[1] = "Smack".into();
    edited[560] = "Storm Bolt II".into();
    catalog::save_text_bank(&bridge, 2, 203, &edited).unwrap();
    assert!(root.join("a/0/0/2/203").is_file());
    let lines = std::fs::read_to_string(root.join("a/0/0/2/203.msg")).unwrap();
    assert!(lines.contains("560 = Storm Bolt II"), "{lines}");
    let after = catalog::text_bank(&bridge, 2, 203).unwrap();
    assert_eq!(after[1], "Smack");
    assert_eq!(after[560], "Storm Bolt II");
}

#[test]
fn a_pokemon_is_added_in_every_archive_and_removed_with_renumbering() {
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    use kenon_core::species_add;
    let root = dir.path().join("m");
    // Pikachu (25) and Bulbasaur (1).
    let a = species_add::add(&bridge, 25, "Voltwing").unwrap();
    let b = species_add::add(&bridge, 1, "Sprouty").unwrap();
    assert_eq!((a, b), (715, 716));
    let bridge = reopen(bridge, &dir);
    let m = |arc: u16, i: usize| bridge.member(Member { arc, index: i }).unwrap();
    // Stats copied, without Pikachu's forms; sprites and icons copied.
    assert_eq!(m(16, 716)[..28], m(16, 1)[..28]);
    assert_eq!(m(16, 716)[32], 1);
    assert_eq!(m(4, 716 * 20 + 3), m(4, 20 + 3));
    assert_eq!(m(7, 7 + 2 * 716), m(7, 7 + 2));
    assert_eq!(m(20, 716), 716u16.to_le_bytes().to_vec());
    assert_eq!(m(19, 716), vec![0u8; 42]);
    let names = catalog::text_bank(&bridge, 2, 70).unwrap();
    assert_eq!(
        (names[715].as_str(), names[716].as_str()),
        ("Voltwing", "Sprouty")
    );
    assert_eq!(catalog::text_bank(&bridge, 2, 284).unwrap()[716], "SPROUTY");
    let manifest = std::fs::read_to_string(root.join("eidos_mod.txt")).unwrap();
    assert!(manifest.contains("limit.species = 716"), "{manifest}");
    // The picture screens list the new Pokemon by name: battle sprites
    // (arc 4), icons (7) and Pokedex icons (167), each at its own member.
    for (arc, chr) in [(4u16, 716 * 20), (7, 7 + 2 * 716), (167, 716 + 5)] {
        let sheets = catalog::named_sheets(&bridge, arc).unwrap();
        let sheets = sheets.as_ref().as_ref().unwrap();
        assert!(
            sheets
                .iter()
                .any(|s| s.label.starts_with("Sprouty") && s.chr == chr),
            "arc {arc}: no Sprouty sheet at {chr}"
        );
    }
    // Its front sprite can be replaced on its own, leaving Bulbasaur's.
    let mut front = m(4, 716 * 20);
    let last = front.len() - 1;
    front[last] ^= 0xFF;
    catalog::write_member(
        &bridge,
        Member {
            arc: 4,
            index: 716 * 20,
        },
        &front,
    )
    .unwrap();
    let bridge = reopen(bridge, &dir);
    let m = |arc: u16, i: usize| bridge.member(Member { arc, index: i }).unwrap();
    assert_eq!(m(4, 716 * 20), front);
    assert_ne!(m(4, 716 * 20), m(4, 20));
    // Bulbasaur's evolution points at the second new one.
    let mut evo = tables::Edit::default();
    evo.lists.insert("rows".into(), vec![vec![4, 16, 716]]);
    tables::save(&bridge, "evolutions", 1, &evo).unwrap();

    species_add::remove(&bridge, 715).unwrap();
    let bridge = reopen(bridge, &dir);
    let m = |arc: u16, i: usize| bridge.member(Member { arc, index: i }).unwrap();
    // Sprouty moved down to 715 everywhere, and the evolution followed.
    assert_eq!(catalog::text_bank(&bridge, 2, 70).unwrap()[715], "Sprouty");
    assert_eq!(m(4, 715 * 20 + 3), m(4, 20 + 3));
    assert_eq!(m(20, 715), 715u16.to_le_bytes().to_vec());
    let e = m(19, 1);
    assert_eq!(u16_at(&e, 4), 715);
    assert!(!root.join("a/0/1/6/716").exists());
    assert_eq!(species_add::added(&bridge), vec![715]);
}

#[test]
fn a_party_kind_change_converts_the_party() {
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    // Trainer 53 (Cheren's first battle): one Tepig, plain party.
    let party = |b: &Bridge| b.member(Member { arc: 93, index: 53 }).unwrap();
    let before = party(&bridge);
    assert_eq!(before.len(), 8);
    let species = u16_at(&before, 4);
    let level = u16_at(&before, 2);
    let mut edit = tables::Edit::default();
    edit.values.insert("partyType".into(), 3);
    tables::save(&bridge, "trainers", 53, &edit).unwrap();
    let after = party(&bridge);
    // Moves and item: 18 bytes; the shared fields kept.
    assert_eq!(after.len(), 18);
    assert_eq!(after[..8], before[..8]);
    assert_eq!(u16_at(&after, 8), 0, "no held item yet");
    // The moves it knows at its level, from its learnset.
    let learn = bridge
        .member(Member {
            arc: 18,
            index: species as usize,
        })
        .unwrap();
    let known: Vec<u16> = learn
        .as_chunks::<4>()
        .0
        .iter()
        .take_while(|p| u16_at(&p[..], 0) != 0xFFFF && u16_at(&p[..], 2) <= level)
        .map(|p| u16_at(p, 0))
        .collect();
    let want: Vec<u16> = known[known.len().saturating_sub(4)..].to_vec();
    let got: Vec<u16> = (0..want.len())
        .map(|k| u16_at(&after, 10 + k * 2))
        .collect();
    assert_eq!(got, want);
    // Back to plain: the moves and item go, the rest stays.
    edit.values.insert("partyType".into(), 0);
    tables::save(&bridge, "trainers", 53, &edit).unwrap();
    assert_eq!(party(&bridge), before);
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

#[test]
fn removing_a_move_renumbers_the_later_ones_and_their_uses() {
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    for (from, name) in [(85, "One"), (1, "Two"), (33, "Three")] {
        content::add(&bridge, "moves", from, name).unwrap();
    }
    let bridge = reopen(bridge, &dir);
    // Bulbasaur learns all three.
    let mut edit = tables::Edit::default();
    edit.lists.insert(
        "rows".into(),
        vec![vec![560, 5], vec![561, 7], vec![562, 9]],
    );
    tables::save(&bridge, "learnsets", 1, &edit).unwrap();

    let removed = content::remove(&bridge, "moves", 560).unwrap();
    assert_eq!(removed.renumbered, vec![(561, 560), (562, 561)]);
    assert_eq!(removed.cleared, vec!["Level-up learnsets 1".to_string()]);
    let bridge = reopen(bridge, &dir);

    // The row of the removed move went; the others follow their moves.
    let rows = bridge.member(Member { arc: 18, index: 1 }).unwrap();
    assert_eq!(
        (
            u16_at(&rows, 0),
            u16_at(&rows, 2),
            u16_at(&rows, 4),
            u16_at(&rows, 6)
        ),
        (560, 7, 561, 9)
    );
    assert_eq!(u16_at(&rows, 8), 0xFFFF);
    // Records, names and the limit moved down; no id is left empty.
    let names = catalog::text_bank(&bridge, 2, 203).unwrap();
    assert_eq!((names[560].as_str(), names[561].as_str()), ("Two", "Three"));
    assert_eq!(names.len(), 562);
    let rec = |i| bridge.member(Member { arc: 21, index: i }).unwrap();
    assert_eq!(rec(560), rec(1));
    assert_eq!(rec(561), rec(33));
    let k = content::kind("moves").unwrap();
    assert_eq!(content::count(&bridge, k).unwrap(), 562);
    let battle = catalog::text_bank(&bridge, 2, 13).unwrap();
    assert!(battle[560 * 3].contains("Two"), "{}", battle[560 * 3]);
    assert!(
        battle[561 * 3 + 2].contains("Three"),
        "{}",
        battle[561 * 3 + 2]
    );
}

#[test]
fn removing_an_item_renumbers_evolutions_held_items_and_item_balls() {
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    for (from, name) in [(17, "A Potion"), (80, "A Stone"), (234, "A Berry")] {
        content::add(&bridge, "items", from, name).unwrap();
    }
    let bridge = reopen(bridge, &dir);
    // Bulbasaur evolves with item 628 and holds item 629.
    let mut evo = tables::Edit::default();
    evo.lists.insert("rows".into(), vec![vec![8, 628, 2]]);
    tables::save(&bridge, "evolutions", 1, &evo).unwrap();
    let mut held = tables::Edit::default();
    held.values.insert("heldItemCommon".into(), 629);
    tables::save(&bridge, "species", 1, &held).unwrap();
    // The first item ball gives item 628.
    kenon_core::fielditems::set_event_item(&bridge, 7000, 628, None).unwrap();

    let k = content::kind("items").unwrap();
    let icon_629 = content::icon_row(&bridge, k, 629).unwrap();
    content::remove(&bridge, "items", 627).unwrap();
    let bridge = reopen(bridge, &dir);
    let evolution = |b: &Bridge| b.member(Member { arc: 19, index: 1 }).unwrap();
    let e = evolution(&bridge);
    assert_eq!((u16_at(&e, 0), u16_at(&e, 2), u16_at(&e, 4)), (8, 627, 2));
    let species = tables::record(&bridge, "species", 1, 0).unwrap();
    let held_now = match &species.sections[0] {
        tables::Section::Fields { values, .. } => values
            .iter()
            .find(|v| v.key == "heldItemCommon")
            .map(|v| v.value),
        _ => None,
    };
    assert_eq!(held_now, Some(628));
    let ball = kenon_core::fielditems::event_item(&bridge, 7000).unwrap();
    assert_eq!(ball.item, 627);
    // The icon rows moved with their items.
    assert_eq!(content::icon_row(&bridge, k, 628).unwrap(), icon_629);

    // Removing the stone too: the evolution that needs it goes, the ball
    // gives nothing.
    let removed = content::remove(&bridge, "items", 627).unwrap();
    assert!(
        removed.cleared.iter().any(|p| p.starts_with("Evolutions")),
        "{removed:?}"
    );
    assert!(
        removed.cleared.iter().any(|p| p.starts_with("Item ball")),
        "{removed:?}"
    );
    let bridge = reopen(bridge, &dir);
    let e = evolution(&bridge);
    assert_eq!((u16_at(&e, 0), u16_at(&e, 2), u16_at(&e, 4)), (0, 0, 0));
    assert_eq!(
        kenon_core::fielditems::event_item(&bridge, 7000)
            .unwrap()
            .item,
        0
    );
    assert_eq!(content::count(&bridge, k).unwrap(), 628);
}

#[test]
fn an_added_ability_works_like_another_and_follows_a_removal() {
    use kenon_core::handlers;
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    // Two new abilities from Intimidate (22): 165 and 166.
    let a = content::add(&bridge, "abilities", 22, "Glare").unwrap();
    let bridge = reopen(bridge, &dir);
    let b = content::add(&bridge, "abilities", 22, "Super Intimidate").unwrap();
    let bridge = reopen(bridge, &dir);
    assert_eq!((a, b), (165, 166));
    let project = bridge.project().unwrap();
    let params = handlers::params("ability", None).unwrap();
    let row = |keys: &[(&str, &str)]| -> handlers::Row {
        vec![handlers::Part {
            base: 22,
            keys: keys
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }]
    };

    // Attack -1 is Intimidate's own value, so only stages is written.
    handlers::set(
        project,
        "ability",
        &params,
        b,
        Some(row(&[("stat", "atk"), ("stages", "-2")])),
    )
    .unwrap();
    handlers::set(project, "ability", &params, a, Some(row(&[]))).unwrap();
    let text =
        std::fs::read_to_string(dir.path().join("m/eidos/tables/ability_handlers.txt")).unwrap();
    assert!(text.contains("165 = 22\n"), "{text}");
    assert!(text.contains("166 = 22 stages=-2\n"), "{text}");
    // A key Intimidate has no place for, and a value out of range, are refused.
    assert!(handlers::set(project, "ability", &params, b, Some(row(&[("hp", "1/2")]))).is_err());
    assert!(handlers::set(
        project,
        "ability",
        &params,
        b,
        Some(row(&[("stages", "9")]))
    )
    .is_err());
    // The same base twice is refused.
    let twice = [row(&[]), row(&[])].concat();
    assert!(handlers::set(project, "ability", &params, b, Some(twice)).is_err());
    // Several bases: tuned Intimidate plus Drizzle (2); Levitate (26) and
    // Truant (54) share work slot 0 and are refused together.
    let part = |base: usize| handlers::Part {
        base,
        keys: Default::default(),
    };
    let both = [row(&[("stages", "-2")]), vec![part(2)]].concat();
    handlers::set(project, "ability", &params, a, Some(both)).unwrap();
    let text =
        std::fs::read_to_string(dir.path().join("m/eidos/tables/ability_handlers.txt")).unwrap();
    assert!(text.contains("165 = 22 stages=-2 + 2\n"), "{text}");
    assert!(handlers::set(
        project,
        "ability",
        &params,
        b,
        Some(vec![part(26), part(54)])
    )
    .is_err());
    // A line that joins several bases cannot be a base itself.
    assert!(handlers::set(project, "ability", &params, b, Some(vec![part(165)])).is_err());
    // Back to plain Intimidate for the removal below.
    handlers::set(project, "ability", &params, a, Some(row(&[]))).unwrap();

    // Removing the first moves the second's row down to 165.
    content::remove(&bridge, "abilities", a).unwrap();
    let rows = handlers::rows(project, "ability");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[&165], row(&[("stages", "-2")]));
}

#[test]
fn a_tm_teaches_another_move_and_its_description_follows() {
    use kenon_core::machines;
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let file = dir.path().join("m/eidos/tables/machine_moves.txt");
    let game = machines::moves(&bridge).unwrap();
    // TM01 (Hone Claws, 468) becomes Flamethrower (53).
    // Machine order: TM01..TM95, then HM01..HM06 (TM93 Wild Charge, HM01 Cut).
    assert_eq!(
        (game[0], game[92], game[95], game[100]),
        (468, 528, 15, 291)
    );
    machines::set(&bridge, 0, 53).unwrap();
    let bridge = reopen(bridge, &dir);
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("TM01 = 53\n"));
    assert_eq!(machines::moves(&bridge).unwrap()[0], 53);
    let items = catalog::text_bank(&bridge, 2, 53).unwrap();
    let moves = catalog::text_bank(&bridge, 2, 202).unwrap();
    assert_eq!(items[328], moves[53]);
    // Back to the game's move: the line goes.
    machines::set(&bridge, 0, 468).unwrap();
    assert!(!file.exists());
}

#[test]
fn an_icon_palette_is_set_for_a_game_pokemon() {
    use kenon_core::species_add;
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let file = dir.path().join("m/eidos/tables/icon_palette.txt");
    let game = species_add::icon_palette(&bridge, 130);
    let (male, female) = (game & 0xF, game >> 4);
    let other = (male + 1) % 3;
    species_add::set_icon_palette(&bridge, 130, other, female).unwrap();
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains(&format!("130 = {other} {female}\n")));
    assert_eq!(
        species_add::icon_palette(&bridge, 130),
        other | (female << 4)
    );
    // The game's own values remove the row.
    species_add::set_icon_palette(&bridge, 130, male, female).unwrap();
    assert!(!file.exists());
}

#[test]
fn a_shop_list_is_changed_and_restored() {
    use kenon_core::shops;
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let file = dir.path().join("m/eidos/tables/shops.txt");
    let game = shops::game(&bridge).unwrap();
    // The main clerk with no badges sells Poke Balls (4) and Potions (17).
    assert_eq!(game.marts.len(), 26);
    assert_eq!(game.marts[0], vec![4, 17]);
    assert_eq!(game.prizes.len(), 3);
    let mut s = shops::current(&bridge).unwrap();
    s.marts[0] = vec![1, 2, 3, 50];
    shops::set(&bridge, &s).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<_>>(),
        vec!["mart.0 = 1 2 3 50"]
    );
    assert_eq!(shops::current(&bridge).unwrap().marts[0], vec![1, 2, 3, 50]);
    // More than 40 items is refused.
    s.marts[1] = vec![17; 41];
    assert!(shops::set(&bridge, &s).is_err());
    // The game's lists again: no file.
    shops::set(&bridge, &game).unwrap();
    assert!(!file.exists());
}

#[test]
fn a_move_animation_follows_its_default_and_a_removal() {
    use kenon_core::{handlers, move_anims};
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let a = content::add(&bridge, "moves", 85, "Spark Burst").unwrap();
    let bridge = reopen(bridge, &dir);
    let b = content::add(&bridge, "moves", 85, "Moonblast").unwrap();
    let bridge = reopen(bridge, &dir);
    assert_eq!((a, b), (560, 561));
    let project = bridge.project().unwrap();
    let params = handlers::params("move", None).unwrap();
    let like = |base: usize| {
        Some(vec![handlers::Part {
            base,
            keys: Default::default(),
        }])
    };

    // Defaults: a game move plays its own, an added one its base's, else Pound's.
    assert_eq!(move_anims::default_for(project, 53), 53);
    handlers::set(project, "move", &params, b, like(85)).unwrap();
    assert_eq!(move_anims::default_for(project, b), 85);
    handlers::set(project, "move", &params, a, None).unwrap();
    assert_eq!(move_anims::default_for(project, a), 1);

    // The default writes no line; others do; values past 559 are refused.
    move_anims::set(project, b, Some(85)).unwrap();
    assert!(move_anims::all(project).is_empty());
    move_anims::set(project, 53, Some(85)).unwrap();
    move_anims::set(project, a, Some(53)).unwrap();
    move_anims::set(project, b, Some(236)).unwrap();
    assert!(move_anims::set(project, b, Some(560)).is_err());
    let text =
        std::fs::read_to_string(dir.path().join("m/eidos/tables/move_anims.txt")).unwrap();
    assert!(text.contains("53 = 85\n560 = 53\n561 = 236\n"), "{text}");

    // Removing the first moves the second's line down to 560.
    content::remove(&bridge, "moves", a).unwrap();
    let all = move_anims::all(project);
    assert_eq!(all.len(), 2);
    assert_eq!((all[&53], all[&560]), (85, 236));
}

#[test]
fn a_new_pokemon_gets_forms_that_move_with_the_species() {
    use kenon_core::cries::{self, Key};
    use kenon_core::species_add;
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let stats = |b: &Bridge, i: usize| b.member(Member { arc: 16, index: i }).unwrap();
    let u16_of = |b: &[u8], at: usize| u16::from_le_bytes([b[at], b[at + 1]]) as usize;

    // Voltwing from Pikachu, with a form made from Raichu (26).
    let a = species_add::add(&bridge, 25, "Voltwing").unwrap();
    let bridge = reopen(bridge, &dir);
    assert_eq!(species_add::add_form(&bridge, a, 26).unwrap(), 1);
    let bridge = reopen(bridge, &dir);
    let l = species_add::layout(&bridge);
    assert_eq!((l.last, l.forms[&715].clone()), (715, vec![716]));
    let s = stats(&bridge, 715);
    assert_eq!((u16_of(&s, 28), u16_of(&s, 30), s[32]), (716, 716, 2));
    assert_eq!(stats(&bridge, 716)[..28], stats(&bridge, 26)[..28]);
    let names = tables::entries(&bridge, "species").unwrap();
    assert!(names.iter().any(|e| e.index == 716 && e.name == "Voltwing (form 1)"));
    // Sprites and Pokedex icons are packed as the game packs them: the
    // battle sprite block's packed members LZ11, raw ones raw, the Pokedex
    // icon LZ10; the form's too. Saving an edit keeps the packing.
    let stored = |arc: u16, i: usize| bridge.member_stored(Member { arc, index: i }).unwrap();
    for k in 0..20 {
        assert_eq!(stored(4, 715 * 20 + k)[0], stored(4, 25 * 20 + k)[0], "slot member {k}");
        assert_eq!(stored(4, 716 * 20 + k)[0], stored(4, 26 * 20 + k)[0], "form member {k}");
    }
    assert_eq!(stored(4, 715 * 20)[0], 0x11);
    assert_eq!(stored(167, 720)[0], 0x10);
    let front = bridge.member(Member { arc: 4, index: 715 * 20 }).unwrap();
    catalog::write_member(&bridge, Member { arc: 4, index: 715 * 20 }, &front).unwrap();
    assert_eq!(stored(4, 715 * 20)[0], 0x11);
    // Pokedex lines past the game's 727: Voltwing (715) at 727 with
    // Pikachu's height, its form (716) at 728 with Raichu's.
    let height = catalog::text_bank(&bridge, 2, 245).unwrap();
    assert_eq!((&height[727], &height[728]), (&height[25], &height[26]));
    assert_eq!(catalog::text_bank(&bridge, 2, 244).unwrap()[727], "");
    // The species' cry is Pikachu's; the form follows it.
    let project = bridge.project().unwrap().clone();
    assert_eq!(cries::current(&project, Key { species: 715, form: 0 }), 25);
    assert_eq!(cries::current(&project, Key { species: 715, form: 1 }), 25);

    // A second species takes 716; the form moves to 717.
    let b = species_add::add(&bridge, 1, "Sprouty").unwrap();
    let bridge = reopen(bridge, &dir);
    assert_eq!(b, 716);
    let l = species_add::layout(&bridge);
    assert_eq!((l.last, l.forms[&715].clone()), (716, vec![717]));
    assert_eq!(u16_of(&stats(&bridge, 715), 28), 717);
    assert_eq!(stats(&bridge, 717)[..28], stats(&bridge, 26)[..28]);
    assert_eq!(stats(&bridge, 716)[..28], stats(&bridge, 1)[..28]);
    let m = |arc: u16, i: usize| bridge.member(Member { arc, index: i }).unwrap();
    assert_eq!(m(4, 717 * 20 + 3), m(4, 26 * 20 + 3));
    assert_eq!(m(7, 7 + 2 * 717), m(7, 7 + 2 * 26));
    assert_eq!(m(20, 717), 715u16.to_le_bytes().to_vec());
    assert_eq!(m(20, 716), 716u16.to_le_bytes().to_vec());
    // The Pokedex lines moved with the slots: Sprouty (716) at 728 with
    // Bulbasaur's weight, the form (717) at 729 with Raichu's.
    let weight = catalog::text_bank(&bridge, 2, 268).unwrap();
    assert_eq!((&weight[728], &weight[729]), (&weight[1], &weight[26]));

    // Form 2 from Sprouty, then form 1 removed: form 2 becomes form 1.
    assert_eq!(species_add::add_form(&bridge, 715, 716).unwrap(), 2);
    let bridge = reopen(bridge, &dir);
    let project = bridge.project().unwrap().clone();
    cries::set(&project, Key { species: 715, form: 2 }, Some(7)).unwrap();
    species_add::remove_form(&bridge, 715, 1).unwrap();
    let bridge = reopen(bridge, &dir);
    let l = species_add::layout(&bridge);
    assert_eq!(l.forms[&715], vec![717]);
    assert_eq!(stats(&bridge, 717)[..28], stats(&bridge, 1)[..28]);
    let project = bridge.project().unwrap().clone();
    assert_eq!(cries::current(&project, Key { species: 715, form: 1 }), 7);

    // Removing Voltwing: Sprouty becomes 715, no forms, slots past it go.
    species_add::remove(&bridge, 715).unwrap();
    let bridge = reopen(bridge, &dir);
    let l = species_add::layout(&bridge);
    assert_eq!((l.last, l.forms.len()), (715, 0));
    assert_eq!(stats(&bridge, 715)[..28], stats(&bridge, 1)[..28]);
    let project = bridge.project().unwrap();
    assert!(project.member(16, 716).is_none() && project.member(16, 717).is_none());
    assert!(cries::all(project).is_empty());
    // One Pokedex line past the game's: Sprouty's at 727.
    let weight = catalog::text_bank(&bridge, 2, 268).unwrap();
    assert_eq!(weight.len(), 728);
    assert_eq!(weight[727], weight[1]);
}

#[test]
fn a_new_pokemon_joins_the_unova_pokedex() {
    use kenon_core::cries::Key;
    use kenon_core::dex_search::{self, Search};
    use kenon_core::regional_dex::{self as dex, Place};
    use kenon_core::species_add;
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    // The game's places: Victini 0, Snivy 1, Genesect 155, Pikachu none.
    assert_eq!(dex::game(&bridge, 494), Place::Number(0));
    assert_eq!(dex::game(&bridge, 495), Place::Number(1));
    assert_eq!(dex::game(&bridge, 649), Place::Number(155));
    assert_eq!(dex::game(&bridge, 25), Place::National);
    // Search values: Pikachu is a quadruped (4) under P (15).
    assert_eq!(dex_search::game(&bridge, 25), Search { shape: Some(4), letter: Some(15) });

    let a = species_add::add(&bridge, 25, "Voltwing").unwrap();
    let bridge = reopen(bridge, &dir);
    let b = species_add::add(&bridge, 1, "Sprouty").unwrap();
    let bridge = reopen(bridge, &dir);
    let project = bridge.project().unwrap().clone();
    // Added: National only, with the source's shape and no letter.
    assert_eq!(dex::current(&bridge, &project, a), Place::National);
    assert_eq!(dex_search::current(&bridge, &project, Key { species: a, form: 0 }).shape, Some(4));
    assert_eq!(dex_search::current(&bridge, &project, Key { species: a, form: 0 }).letter, None);

    // Both in the Unova Pokedex: numbered after Genesect, in id order.
    dex::set(&bridge, &project, a, Place::Unova).unwrap();
    dex::set(&bridge, &project, b, Place::Unova).unwrap();
    let n = dex::numbers(&bridge, &project);
    assert_eq!((n[&a], n[&b]), (156, 157));
    // Pikachu gets its own number; Snivy leaves; Victini keeps its own.
    dex::set(&bridge, &project, 25, Place::Number(200)).unwrap();
    dex::set(&bridge, &project, 495, Place::National).unwrap();
    dex::set(&bridge, &project, 494, Place::Number(0)).unwrap();
    assert!(dex::set(&bridge, &project, a, Place::Number(999)).is_err());
    let text =
        std::fs::read_to_string(dir.path().join("m/eidos/tables/regional_dex.txt")).unwrap();
    assert!(text.contains("25 = 200\n495 = none\n715 = unova\n716 = unova\n"), "{text}");
    let n = dex::numbers(&bridge, &project);
    assert_eq!((n[&a], n[&b]), (201, 202));
    // A number shared with another species is reported.
    dex::set(&bridge, &project, b, Place::Number(200)).unwrap();
    assert_eq!(dex::info(&bridge, &project, b).shared_with, Some(25));

    // Removing Voltwing moves Sprouty's lines to 715.
    species_add::remove(&bridge, a).unwrap();
    let all = dex::all(&project);
    assert_eq!(all.get(&715), Some(&Place::Number(200)));
    assert!(!all.contains_key(&716));
    let s = dex_search::all(&project);
    assert_eq!(s.get(&Key { species: 715, form: 0 }).and_then(|s| s.shape), Some(4));
    assert!(!s.contains_key(&Key { species: 716, form: 0 }));
}

#[test]
fn every_battle_effect_script_reads_back_byte_for_byte() {
    use kenon_core::effscript;
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    // a/0/6/6: move scripts; a/0/6/7: status, weather and item effects.
    let mut runs_off = Vec::new();
    let mut ends = std::collections::BTreeSet::new();
    let (mut total, mut left) = (0, 0);
    for arc in [66u16, 67] {
        let n = bridge.base_archive(arc).unwrap().len();
        for index in 0..n {
            let b = bridge.member(Member { arc, index }).unwrap();
            let text = effscript::disassemble(&b).unwrap_or_else(|e| panic!("{arc}/{index}: {e}"));
            let back = effscript::assemble(&text).unwrap_or_else(|e| panic!("{arc}/{index}: {e}\n{text}"));
            assert_eq!(back, b, "{arc}/{index} differs:\n{text}");
            let (ok, e, raw) = effscript::check(&b);
            // The 360-byte fillers after the last move (560..600) hold no
            // script.
            if !ok && !(arc == 66 && index >= 560) {
                runs_off.push(format!("{arc}/{index}"));
            }
            // Only the 4-byte alignment pad after the last command is left:
            // the command table decodes every byte that runs.
            assert!(raw <= 3, "{arc}/{index}: {raw} bytes undecoded");
            ends.extend(e);
            total += b.len();
            left += raw;
        }
    }
    println!("stop ids {ends:?}; {left} of {total} bytes undecoded; off the end: {runs_off:?}");
    assert!(runs_off.is_empty(), "scripts that run off: {runs_off:?}");
    // The game's scripts end on a release, a return or a jump, never on a
    // stop id.
    assert!(ends.is_empty(), "stop ids {ends:?}");
}

#[test]
fn a_new_move_gets_its_own_animation_script() {
    use kenon_core::move_anims as anims;
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let a = content::add(&bridge, "moves", 85, "Spark Burst").unwrap();
    let bridge = reopen(bridge, &dir);
    let b = content::add(&bridge, "moves", 85, "Moonblast").unwrap();
    let bridge = reopen(bridge, &dir);
    let project = bridge.project().unwrap().clone();
    assert_eq!((a, b), (560, 561));
    // No script yet: none to show, and another move cannot play it.
    assert!(anims::script_text(&bridge, b).unwrap().is_none());
    assert!(anims::set(&project, a, Some(b as u16)).is_err());

    // Moonblast gets a copy of Thunderbolt's script, then an edit.
    anims::copy_script(&bridge, b, 85).unwrap();
    assert_eq!(anims::default_for(&project, b), b as u16);
    let text = anims::script_text(&bridge, b).unwrap().unwrap();
    assert_eq!(text, anims::script_text(&bridge, 85).unwrap().unwrap());
    let edited = text.replacen("WaitFrames 20", "WaitFrames 30", 1);
    anims::set_script(&bridge, b, &edited).unwrap();
    assert_eq!(anims::script_text(&bridge, b).unwrap().unwrap(), edited);
    assert!(anims::set_script(&bridge, b, "variant @x\n@x:\n    WaitFrames\n").is_err());
    // Spark Burst plays Moonblast's script.
    anims::set(&project, a, Some(b as u16)).unwrap();
    let text_file = std::fs::read_to_string(dir.path().join("m/eidos/tables/move_anims.txt")).unwrap();
    assert!(text_file.contains("560 = 561\n"), "{text_file}");

    // Removing Spark Burst moves Moonblast's script to 560.
    content::remove(&bridge, "moves", a).unwrap();
    let bridge = reopen(bridge, &dir);
    let project = bridge.project().unwrap().clone();
    assert!(anims::has_own(&project, 560) && !anims::has_own(&project, 561));
    assert_eq!(anims::script_text(&bridge, 560).unwrap().unwrap(), edited);
    assert!(anims::all(&project).is_empty());

    // Dropping it: the move falls back to its default.
    anims::drop_script(&bridge, 560).unwrap();
    assert!(!anims::has_own(&project, 560));
}

#[test]
fn a_new_type_joins_the_chart_and_follows_a_removal() {
    use kenon_core::types;
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    let (fire, grass, psychic) = (9usize, 11usize, 13usize);
    assert_eq!(types::count(&bridge), 17);
    // An edit of the game's chart: Fire on Grass becomes normal.
    let mut chart = types::chart(&bridge).unwrap();
    assert_eq!(chart[fire][grass], 8);
    chart[fire][grass] = 4;
    types::set_chart(&bridge, &chart).unwrap();
    let file = dir.path().join("m/eidos/tables/type_chart.txt");
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(text.lines().filter(|l| !l.starts_with('#')).count(), 1, "{text}");
    assert!(text.contains("9 = "), "{text}");

    // Fairy, matching up like Psychic.
    let fairy = types::add(&bridge, "Fairy", psychic).unwrap();
    let bridge = reopen(bridge, &dir);
    assert_eq!((fairy, types::count(&bridge)), (17, 18));
    let chart = types::chart(&bridge).unwrap();
    assert_eq!(chart.len(), 18);
    assert!(chart.iter().all(|r| r.len() == 18));
    assert_eq!(chart[fairy][..17], chart[psychic][..17]);
    assert_eq!(chart[fire][fairy], chart[fire][psychic]);
    assert_eq!(chart[fire][grass], 4);
    assert_eq!(tables::enum_values(&bridge, "type").unwrap()[fairy], "Fairy");
    let manifest = std::fs::read_to_string(dir.path().join("m/eidos_mod.txt")).unwrap();
    assert!(manifest.contains("limit.types = 17"), "{manifest}");

    // Pikachu becomes Fairy; removing Fairy makes it Normal (0) again.
    let edit = tables::Edit {
        values: [("type1".to_string(), fairy as i64)].into_iter().collect(),
        ..Default::default()
    };
    tables::save(&bridge, "species", 25, &edit).unwrap();
    types::remove(&bridge, fairy).unwrap();
    let bridge = reopen(bridge, &dir);
    assert_eq!(types::count(&bridge), 17);
    let rec = bridge.member(Member { arc: 16, index: 25 }).unwrap();
    assert_eq!(rec[6], 0);
    let chart = types::chart(&bridge).unwrap();
    assert_eq!((chart.len(), chart[fire][grass]), (17, 4));
    assert!(!std::fs::read_to_string(dir.path().join("m/eidos_mod.txt")).unwrap().contains("limit.types"));
}

#[test]
fn a_table_grid_edits_many_entries_at_once() {
    use kenon_core::grid::{self, Cell};
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    assert!(grid::supported("species") && grid::supported("moves") && !grid::supported("learnsets"));
    let g = grid::grid(&bridge, "species").unwrap();
    let hp = g.columns.iter().position(|c| c.key == "baseHp").unwrap();
    let pikachu = g.rows.iter().find(|r| r.index == 25).unwrap();
    assert_eq!(pikachu.values[hp], 35);
    assert!(g.rows.len() >= 649);
    // Two entries in one save, and a value past the field's range clamped.
    let cells = [
        Cell { index: 25, key: "baseHp".into(), value: 60 },
        Cell { index: 26, key: "baseHp".into(), value: 999 },
    ];
    assert_eq!(grid::set_cells(&bridge, "species", &cells).unwrap(), 2);
    let g = grid::grid(&bridge, "species").unwrap();
    let hp_of = |i: usize| g.rows.iter().find(|r| r.index == i).unwrap().values[hp];
    assert_eq!((hp_of(25), hp_of(26)), (60, 255));
}

#[test]
fn a_type_icon_is_spelled_with_the_games_letters() {
    use kenon_core::type_pictures as pics;
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    // Rebuilding the game's own icons from their names gives them back:
    // the letters, the spacing, the centring and the shadow are the game's.
    let names = tables::enum_values(&bridge, "type").unwrap();
    let mut same = Vec::new();
    for t in 0..17 {
        let game = pics::icon_of(&bridge, t).unwrap();
        let Ok(made) = pics::make_icon(&bridge, t, &names[t]) else {
            continue;
        };
        let border = game[2 * 32];
        let map = |v: u8| match v {
            0 | 14 | 15 => v,
            v if v == border => 2,
            _ => 1,
        };
        if game.iter().map(|&v| map(v)).collect::<Vec<_>>() == made {
            same.push(names[t].clone());
        }
    }
    println!("rebuilt exactly: {same:?}");
    assert!(same.iter().any(|n| n.eq_ignore_ascii_case("fire")), "{same:?}");

    // Fairy from Psychic: every letter is in the game's icons.
    assert!(pics::missing_letters(&bridge, "Fairy").unwrap().is_empty());
    assert_eq!(pics::missing_letters(&bridge, "Jazz").unwrap(), vec!['J', 'Z']);
    let fairy = kenon_core::types::add(&bridge, "Fairy", 13).unwrap();
    let bridge = reopen(bridge, &dir);
    let look = pics::create(&bridge, fairy, 13, "Fairy", None).unwrap();
    let bridge = reopen(bridge, &dir);
    assert_eq!(look.button.len(), 6);
    let icon = pics::icon_of(&bridge, fairy).unwrap();
    let png = kenon_core::image::encode_png(&kenon_core::image::Rgba {
        width: 32,
        height: 16,
        pixels: pics::rgba(&icon, &pics::palette_of(&bridge, fairy)),
    });
    let out = std::path::Path::new("E:/dev/research/type-icons");
    if out.parent().is_some_and(|p| p.is_dir()) {
        std::fs::create_dir_all(out).unwrap();
        std::fs::write(out.join("fairy.png"), png).unwrap();
    }
    // Pink: the colours change, the icon keeps its pixels.
    pics::set_colors(&bridge, fairy, "F6A5D5", "9C4A7B").unwrap();
    let text = std::fs::read_to_string(dir.path().join("m/eidos/tables/types.txt")).unwrap();
    assert!(text.contains("17 = icon ") && text.contains("colors F6A5D5 9C4A7B"), "{text}");
}

#[test]
fn every_particle_resource_reads_back_byte_for_byte() {
    use kenon_core::spa;
    let Some((bridge, _dir)) = bridge() else {
        return;
    };
    let n = bridge.base_archive(6).unwrap().len();
    assert_eq!(n, 733);
    let (mut emitters, mut textures, mut flagged) = (0, 0, Vec::new());
    for index in 0..n {
        let b = bridge.member(Member { arc: 6, index }).unwrap();
        let s = spa::parse(&b).unwrap_or_else(|e| panic!("6/{index}: {e}"));
        assert_eq!(spa::build(&s), b, "6/{index} differs");
        emitters += s.resources.len();
        // Every texture decodes, and re-encoding keeps every pixel's alpha.
        for (t, block) in s.textures.iter().enumerate() {
            let (info, rgba) = spa::decode_texture(block).unwrap_or_else(|e| panic!("6/{index} texture {t}: {e}"));
            assert_eq!(rgba.len(), info.width * info.height * 4);
            let again = spa::encode_texture(block, info.width, info.height, &rgba).unwrap();
            let (_, back) = spa::decode_texture(&again).unwrap();
            let alpha_same = rgba.chunks(4).zip(back.chunks(4)).all(|(a, b)| a[3] == b[3]);
            assert!(alpha_same, "6/{index} texture {t}: alpha changed");
            textures += 1;
        }
        if !spa::problems(&s).is_empty() {
            flagged.push(index);
        }
    }
    println!("{emitters} emitters, {textures} textures; flagged by the checks: {flagged:?}");
    // The game's own files pass kenon's checks.
    assert!(flagged.is_empty(), "{flagged:?}");
}

#[test]
fn a_particle_file_is_copied_and_edited() {
    use kenon_core::spa::{self, Edit};
    let Some((bridge, dir)) = bridge() else {
        return;
    };
    // Thunderbolt (85) loads particles 249 and 250.
    let users = spa::users(&bridge);
    assert!(users[&249].contains(&85) && users[&250].contains(&85));
    let list = spa::list(&bridge).unwrap();
    assert_eq!(list.len(), 733);

    // A copy of 249 becomes file 733.
    let at = spa::add(&bridge, 249).unwrap();
    let bridge = reopen(bridge, &dir);
    assert_eq!(at, 733);
    assert_eq!(spa::load(&bridge, at).unwrap(), spa::load(&bridge, 249).unwrap());

    // Gravity on, then pulled down; the particle life doubled.
    let mut s = spa::load(&bridge, at).unwrap();
    let life = spa::view(&s).emitters[0].values["life"];
    let has_gravity = s.resources[0].blocks.iter().any(|(n, _)| n == "gravity");
    let blocks = if has_gravity { vec![] } else { vec![(0, "gravity".to_string(), true)] };
    let edits = vec![
        Edit { emitter: 0, key: "gravityY".into(), value: -100 },
        Edit { emitter: 0, key: "life".into(), value: life * 2 },
    ];
    spa::apply(&mut s, &blocks, &edits).unwrap();
    spa::store(&bridge, at, &s).unwrap();
    let back = spa::view(&spa::load(&bridge, at).unwrap());
    assert_eq!((back.emitters[0].values["gravityY"], back.emitters[0].values["life"]), (-100, life * 2));
    // An interval of 0 is refused.
    let mut s = spa::load(&bridge, at).unwrap();
    assert!(spa::apply(&mut s, &[], &[Edit { emitter: 0, key: "interval".into(), value: 0 }]).is_err());

    // A texture replaced in its own format keeps its size and format.
    let s = spa::load(&bridge, at).unwrap();
    let (info, rgba) = spa::decode_texture(&s.textures[0]).unwrap();
    let flipped: Vec<u8> = rgba.chunks(4).rev().flatten().copied().collect();
    let block = spa::encode_texture(&s.textures[0], info.width, info.height, &flipped).unwrap();
    assert_eq!(spa::tex_info(&block), info);
}
