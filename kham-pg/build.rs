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

    println!("cargo:rerun-if-changed=src/shim.c");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PG_CONFIG");

    cc::Build::new()
        .file("src/shim.c")
        .include(&includedir_server)
        .include(&includedir)
        // Suppress warnings that are common in PG extension code
        .flag("-Wno-unused-parameter")
        .flag("-Wno-declaration-after-statement")
        .flag("-Wno-missing-field-initializers")
        .compile("kham_pg_shim");
}
