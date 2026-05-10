fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_GUI");
    if std::env::var_os("CARGO_FEATURE_GUI").is_some() {
        tauri_build::build()
    }
}
