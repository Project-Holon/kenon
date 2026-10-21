fn main() {
    // The icons are built into the executable: rebuild when they change.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
