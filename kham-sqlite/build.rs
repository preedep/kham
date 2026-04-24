use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/shim.c");
    println!("cargo:rerun-if-changed=build.rs");

    let mut build = cc::Build::new();
    build.file("src/shim.c").flag("-Wno-unused-parameter");

    // Locate SQLite headers. Priority:
    // 1. SQLITE_INCLUDE_DIR env var (user override)
    // 2. macOS: Xcode/CLT SDK via xcrun
    // 3. Homebrew sqlite (macOS)
    // 4. pkg-config (Linux)
    // 5. /usr/include (Linux fallback)
    if let Ok(dir) = std::env::var("SQLITE_INCLUDE_DIR") {
        build.include(&dir);
    } else {
        #[cfg(target_os = "macos")]
        {
            // Prefer Xcode/CLT SDK (always has sqlite3ext.h on macOS)
            if let Ok(out) = Command::new("xcrun").args(["--show-sdk-path"]).output() {
                if out.status.success() {
                    let sdk = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    build.include(format!("{sdk}/usr/include"));
                }
            } else if let Ok(out) = Command::new("brew").args(["--prefix", "sqlite"]).output() {
                if out.status.success() {
                    let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    build.include(format!("{prefix}/include"));
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Try pkg-config first, fall back to /usr/include
            let pkg_flags = Command::new("pkg-config")
                .args(["--cflags-only-I", "sqlite3"])
                .output();
            if let Ok(out) = pkg_flags {
                if out.status.success() {
                    let flags = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    for flag in flags.split_whitespace() {
                        if let Some(path) = flag.strip_prefix("-I") {
                            build.include(path);
                        }
                    }
                } else {
                    build.include("/usr/include");
                }
            } else {
                build.include("/usr/include");
            }
        }
    }

    build.compile("kham_sqlite_shim");

    // On macOS the cdylib linker rejects unresolved symbols by default.
    // SQLite itself resolves at dlopen time, so permit undefined references.
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-cdylib-link-arg=-undefined");
        println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
    }
}
