//! Makes a project from a ROM hack patch:
//! `import_patch <rom.nds> <patch.xdelta> <project folder> [<name>]`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: import_patch <rom.nds> <patch.xdelta> <project folder> [<name>]");
        std::process::exit(2);
    }
    let name = args.get(3).cloned().unwrap_or_else(|| "Imported".into());
    let t = std::time::Instant::now();
    match kenon_core::romhack::import_patch(
        args[0].as_ref(),
        args[1].as_ref(),
        args[2].as_ref(),
        &name,
    ) {
        Ok(r) => {
            println!(
                "{} in {:.2?}",
                serde_json::to_string_pretty(&r).unwrap(),
                t.elapsed()
            );
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
