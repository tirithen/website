use std::{fs, path::PathBuf};

use anyhow::Result;
use walkdir::WalkDir;
use xxhash_rust::xxh64::xxh64;

pub fn write_files_and_manifest() -> Result<()> {
    let asset_dir = PathBuf::from("assets/");
    let hashed_dir = PathBuf::from("target/assets_hashed/");
    std::fs::create_dir_all(&hashed_dir)?;

    let mut manifest = String::from(
        "pub static ASSET_MANIFEST: phf::Map<&'static str, &'static str> = phf::phf_map! {\n",
    );
    fs::create_dir_all(&hashed_dir)?;

    for entry in WalkDir::new(&asset_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let content = fs::read(entry.path())?;
            let hash = xxh64(&content, 0);
            let original_path = entry
                .path()
                .strip_prefix(&asset_dir)?
                .to_str()
                .unwrap()
                .replace('\\', "/");
            let hashed_name = format!(
                "{}.{:x}.{}",
                entry.path().file_stem().unwrap().to_str().unwrap(),
                hash,
                entry.path().extension().unwrap().to_str().unwrap()
            );
            fs::copy(entry.path(), hashed_dir.join(&hashed_name))?;
            manifest.push_str(&format!("    {:?} => {:?},\n", original_path, hashed_name));
        }
    }

    manifest.push_str("};\n");
    fs::write(
        PathBuf::from("target/generated_asset_manifest.rs"),
        manifest,
    )?;

    Ok(())
}
