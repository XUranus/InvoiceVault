fn main() {
    // On non-Linux platforms, ONNX Runtime is dynamically linked.
    // Find the cached ONNX Runtime dylib and copy it to resources/
    // so Tauri's bundler can include it.
    #[cfg(not(target_os = "linux"))]
    {
        use std::path::PathBuf;

        let lib_name = if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "onnxruntime.dll"
        };

        // Try to find the dylib in the ort cache directory
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
        let ort_cache = PathBuf::from(&home).join(".cache/ort.pyke.io/dfbin");

        let target_triple = if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "aarch64-apple-darwin"
            } else {
                "x86_64-apple-darwin"
            }
        } else if cfg!(target_os = "windows") {
            "x86_64-pc-windows-msvc"
        } else {
            return;
        };

        let target_dir = ort_cache.join(target_triple);
        if !target_dir.exists() {
            println!("cargo:warning=ort cache not found at {}", target_dir.display());
            return;
        }

        // Find the hash directory (there should be exactly one)
        let real_path = std::fs::read_dir(&target_dir)
            .ok()
            .and_then(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .map(|e| e.path().join(lib_name))
            })
            .filter(|p| p.exists());

        let Some(real_path) = real_path else {
            println!("cargo:warning={lib_name} not found in {}", target_dir.display());
            return;
        };

        // Copy to src-tauri/resources/ so Tauri can bundle it
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let resources_dir = manifest_dir.join("resources");
        std::fs::create_dir_all(&resources_dir).unwrap();
        let dest = resources_dir.join(lib_name);

        let should_copy = if dest.exists() {
            let src_modified = std::fs::metadata(&real_path).and_then(|m| m.modified()).ok();
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
                println!("cargo:warning=Bundled {lib_name} from {}", real_path.display());
            }
        }

        println!("cargo:rerun-if-changed={}", real_path.display());
    }
}
