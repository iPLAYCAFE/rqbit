fn main() {
    // Package version comes from workspace (`version.workspace = true`).
    println!(
        "cargo:rustc-env=RQBIT_VERSION={}",
        env!("CARGO_PKG_VERSION")
    );
    tauri_build::build()
}
