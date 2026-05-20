fn main() {
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-arg-bin=invoicevault=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=invoicevault=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }

    // On non-Linux platforms, ONNX Runtime is dynamically loaded at runtime.
    // If a local DLL/dylib is supplied with ORT_DYLIB_PATH, copy it to
    // resources/ so Tauri can bundle it.
    #[cfg(not(target_os = "linux"))]
    {
        use std::path::PathBuf;

        let lib_name = if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "onnxruntime.dll"
        };

        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let resources_dir = if cfg!(target_os = "windows") {
            manifest_dir.join("resources").join("win-x86_64")
        } else {
            manifest_dir.join("resources")
        };
        std::fs::create_dir_all(&resources_dir).unwrap();
        let dest = resources_dir.join(lib_name);
        if dest.exists() {
            println!("cargo:rerun-if-changed={}", dest.display());
            return;
        }

        let real_path = std::env::var_os("ORT_DYLIB_PATH")
            .map(PathBuf::from)
            .filter(|path| path.exists());
        let Some(real_path) = real_path else {
            println!(
                "cargo:warning={lib_name} not found. Put it in src-tauri/resources/win-x86_64 on Windows, src-tauri/resources on macOS, or set ORT_DYLIB_PATH before building."
            );
            return;
        };

        let should_copy = if dest.exists() {
            let src_modified = std::fs::metadata(&real_path)
                .and_then(|m| m.modified())
                .ok();
            let dst_modified = std::fs::metadata(&dest).and_then(|m| m.modified()).ok();
            match (src_modified, dst_modified) {
                (Some(s), Some(d)) => s > d,
                _ => true,
            }
        } else {
            true
        };

        if should_copy {
            if let Err(e) = std::fs::copy(&real_path, &dest) {
                println!("cargo:warning=Failed to copy {lib_name}: {e}");
            } else {
                println!(
                    "cargo:warning=Bundled {lib_name} from {}",
                    real_path.display()
                );
            }
        }

        println!("cargo:rerun-if-changed={}", real_path.display());
    }

    tauri_build::build()
}
