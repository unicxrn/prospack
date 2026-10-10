fn main() {
    // The setup app embeds the real installer; dev builds work without one.
    let payload = std::path::Path::new("payload/prospack-setup.exe");
    if !payload.exists() {
        std::fs::create_dir_all("payload").unwrap();
        std::fs::write(payload, b"").unwrap();
    }
    if !std::path::Path::new("payload/size.txt").exists() {
        std::fs::write("payload/size.txt", "0").unwrap();
    }
    println!("cargo:rerun-if-changed=payload/prospack-setup.exe");
    println!("cargo:rerun-if-changed=payload/size.txt");
    tauri_build::build()
}
