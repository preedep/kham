use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/shim.c");
    println!("cargo:rerun-if-changed=build.rs");

    let mut build = cc::Build::new();
    build.file("src/shim.c").flag("-Wno-unused-parameter");

    // Locate SQLite headers. Priority:
    // 1. SQLITE_INCLUDE_DIR env var (user override — works on all platforms)
    // 2. macOS: Xcode/CLT SDK via xcrun, then Homebrew sqlite
    // 3. Android: NDK sysroot (sqlite3.h only — sqlite3ext.h requires SQLITE_INCLUDE_DIR)
    // 4. Windows: vcpkg (VCPKG_ROOT env, then C:/vcpkg default)
    // 5. Linux: pkg-config sqlite3, then /usr/include
    //
    // Android note: sqlite3ext.h is not in the NDK sysroot. For Android builds,
    // set SQLITE_INCLUDE_DIR to the SQLite amalgamation directory which contains
    // both sqlite3.h and sqlite3ext.h (https://www.sqlite.org/download.html).
    // CI downloads this automatically via the release workflow.
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

        #[cfg(target_os = "android")]
        {
            // NDK sysroot provides sqlite3.h; sqlite3ext.h must come from
            // SQLITE_INCLUDE_DIR (SQLite amalgamation). Without it the build fails.
            let ndk = std::env::var("ANDROID_NDK_LATEST_HOME")
                .or_else(|_| std::env::var("ANDROID_NDK_ROOT"))
                .or_else(|_| std::env::var("ANDROID_NDK_HOME"))
                .unwrap_or_default();
            if !ndk.is_empty() {
                build.include(format!(
                    "{ndk}/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/include"
                ));
            }
        }

        #[cfg(target_os = "windows")]
        {
            // vcpkg is the standard way to get SQLite headers on Windows.
            // GitHub Actions runners have vcpkg at C:/vcpkg by default.
            let vcpkg_root = std::env::var("VCPKG_ROOT").unwrap_or_else(|_| "C:/vcpkg".to_string());
            build.include(format!("{vcpkg_root}/installed/x64-windows/include"));
        }

        #[cfg(all(
            not(target_os = "macos"),
            not(target_os = "android"),
            not(target_os = "windows")
        ))]
        {
            // Linux: try pkg-config first, fall back to /usr/include
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
