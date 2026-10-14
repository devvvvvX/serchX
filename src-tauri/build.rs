fn main() {
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=../ui");

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("clang")
            .args(["-print-file-name=libclang_rt.osx.a"])
            .output()
        {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let path = std::path::Path::new(&path_str);
            if path.exists() {
                println!("cargo:rustc-link-arg={}", path.display());
            }
        }
    }

    tauri_build::build()
}
