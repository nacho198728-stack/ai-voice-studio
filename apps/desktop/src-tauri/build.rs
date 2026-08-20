fn main() {
    generate_icon();
    let attributes = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    tauri_build::try_build(attributes).expect("build Tauri desktop resources");
    embed_windows_manifest_for_all_targets();
}

fn embed_windows_manifest_for_all_targets() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }

    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    println!("cargo:rustc-link-arg=/WX");
}

fn generate_icon() {
    const SIZE: u32 = 64;
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let directory = manifest.join("icons");
    let path = directory.join("icon.png");
    std::fs::create_dir_all(&directory).expect("create generated desktop icon directory");

    let file = std::fs::File::create(path).expect("create generated desktop icon");
    let mut encoder = png::Encoder::new(file, SIZE, SIZE);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("write desktop icon header");
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let inset = (10..54).contains(&x) && (10..54).contains(&y);
            let accent = (18..28).contains(&x) || (36..46).contains(&x);
            let (red, green, blue) = if inset && accent {
                (158, 140, 255)
            } else if inset {
                (43, 39, 63)
            } else {
                (16, 17, 19)
            };
            pixels.extend_from_slice(&[red, green, blue, 255]);
        }
    }
    writer
        .write_image_data(&pixels)
        .expect("write generated desktop icon pixels");
}
