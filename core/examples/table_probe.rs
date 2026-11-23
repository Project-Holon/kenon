//! Prints every record of a table with its enum names:
//! `table_probe <rom> <table id> [<project folder>]`.

use std::sync::Arc;

use kenon_core::bridge::Bridge;
use kenon_core::project::Project;
use kenon_core::rom::Rom;
use kenon_core::tables::{self, Section};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let project = a
        .get(3)
        .map(|p| Arc::new(Project::open(p.as_ref()).unwrap()));
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap()).with_project(project);
    let t = tables::table(&a[2]).unwrap();
    let entries = tables::entries(&bridge, &a[2]).unwrap();
    for e in entries.iter() {
        let r = tables::record(&bridge, &a[2], e.index, 0).unwrap();
        let mut line = format!("{:>3} {}:", e.index, e.name);
        for s in &r.sections {
            if let Section::Fields { values, .. } = s {
                for v in values {
                    let f = t.fields.iter().find(|f| f.name == v.key);
                    let named = f
                        .and_then(|f| f.enum_name.as_deref())
                        .and_then(|n| tables::enum_values(&bridge, n).ok())
                        .and_then(|names| names.get(v.value as usize).cloned())
                        .filter(|s| !s.is_empty());
                    match named {
                        Some(n) => line.push_str(&format!(" {}={n}", v.key)),
                        None => line.push_str(&format!(" {}={}", v.key, v.value)),
                    }
                }
            }
        }
        println!("{line}");
    }
}
