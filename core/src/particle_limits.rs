//! How many emitters and particles a particle file may have alive at once
//! (eidos `eidos/tables/particles.txt`):
//!
//! ```text
//! 312 = emitters 40 particles 600
//! ```
//!
//! The key is the a/0/0/6 member. Either key may be left out: the game's
//! 24 emitters and 200 particles. A lower value plays as the default, so
//! kenon writes only values above it.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::project::Project;

const TABLE: &str = "particles";
/// The game's limits per file.
pub const EMITTERS: u32 = 24;
pub const PARTICLES: u32 = 200;
/// eidos's ceilings.
pub const MAX_EMITTERS: u32 = 1000;
pub const MAX_PARTICLES: u32 = 10000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Limits {
    pub emitters: u32,
    pub particles: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            emitters: EMITTERS,
            particles: PARTICLES,
        }
    }
}

fn parse(line: &str) -> Limits {
    let words: Vec<&str> = line
        .split('#')
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect();
    let mut l = Limits::default();
    for pair in words.chunks(2) {
        let v = pair.get(1).and_then(|v| v.parse::<u32>().ok());
        match (pair[0], v) {
            ("emitters", Some(v)) => l.emitters = v.clamp(1, MAX_EMITTERS).max(EMITTERS),
            ("particles", Some(v)) => l.particles = v.clamp(1, MAX_PARTICLES).max(PARTICLES),
            _ => {}
        }
    }
    l
}

fn format(l: Limits) -> Option<String> {
    let mut parts = Vec::new();
    if l.emitters > EMITTERS {
        parts.push(format!("emitters {}", l.emitters));
    }
    if l.particles > PARTICLES {
        parts.push(format!("particles {}", l.particles));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// A file's limits.
pub fn get(project: &Project, file: usize) -> Limits {
    project
        .table_lines(TABLE)
        .get(&file)
        .map(|l| parse(l))
        .unwrap_or_default()
}

/// Sets a file's limits; the defaults take its line out.
pub fn set(project: &Project, file: usize, l: Limits) -> Result<(), String> {
    if !(1..=MAX_EMITTERS).contains(&l.emitters) || !(1..=MAX_PARTICLES).contains(&l.particles) {
        return Err(format!(
            "emitters go up to {MAX_EMITTERS} and particles up to {MAX_PARTICLES}"
        ));
    }
    let mut lines: BTreeMap<usize, String> = project.table_lines(TABLE);
    match format(l) {
        Some(s) => {
            lines.insert(file, s);
        }
        None => {
            lines.remove(&file);
        }
    }
    project
        .write_table_lines(
            TABLE,
            &lines,
            "<a/0/0/6 member> = emitters <24..1000> particles <200..10000>: how many may be alive at once.",
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_defaults() {
        assert_eq!(
            parse("emitters 40 particles 600"),
            Limits {
                emitters: 40,
                particles: 600
            }
        );
        assert_eq!(
            parse("particles 900"),
            Limits {
                emitters: 24,
                particles: 900
            }
        );
        assert_eq!(
            parse("emitters 3"),
            Limits::default(),
            "lower plays as the default"
        );
        let dir = tempfile::tempdir().unwrap();
        let project = Project::create(&dir.path().join("m"), "Test", None).unwrap();
        set(
            &project,
            312,
            Limits {
                emitters: 40,
                particles: 200,
            },
        )
        .unwrap();
        assert_eq!(project.table_lines(TABLE)[&312], "emitters 40");
        assert_eq!(get(&project, 312).emitters, 40);
        set(&project, 312, Limits::default()).unwrap();
        assert!(project.table_lines(TABLE).is_empty());
        assert!(set(
            &project,
            1,
            Limits {
                emitters: 2000,
                particles: 200
            }
        )
        .is_err());
    }
}
