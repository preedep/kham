use std::process::Command;

fn pg_config(arg: &str) -> String {
    let exe = std::env::var("PG_CONFIG").unwrap_or_else(|_| "pg_config".to_string());
    let out = Command::new(&exe)
        .arg(arg)
        .output()
        .unwrap_or_else(|_| panic!("Failed to run `{exe} {arg}`. Set PG_CONFIG env var."));
    String::from_utf8(out.stdout)
        .expect("pg_config output is not valid UTF-8")
        .trim()
        .to_string()
}

fn main() {
    let includedir_server = pg_config("--includedir-server");
    let includedir = pg_config("--includedir");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    println!("cargo:rerun-if-changed=src/shim.c");
    println!("cargo:rerun-if-changed=src/pg_exports.map");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PG_CONFIG");

    cc::Build::new()
        .file("src/shim.c")
        .include(&includedir_server)
        .include(&includedir)
        // Default symbol visibility so PG_MODULE_MAGIC / Pg_magic_func and
        // all PG_FUNCTION_INFO_V1 symbols compile as T (global) in the object.
        // The version script below then controls what goes into the dynamic table.
        .flag("-fvisibility=default")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-declaration-after-statement")
        .flag("-Wno-missing-field-initializers")
        .compile("kham_pg_shim");

    // On Linux, Rust cdylib uses --version-script which hides all C symbols
    // by default (local: *).  Provide our own script that explicitly exports
    // the nine symbols PostgreSQL resolves via dlsym at extension load time.
    // macOS does not use version scripts; -fvisibility=default above is enough.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        println!(
            "cargo:rustc-link-arg=-Wl,--version-script={manifest_dir}/src/pg_exports.map"
        );
    }
}
