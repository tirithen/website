mod assets_build {
    include!("src/assets_build.rs");
}

fn main() -> anyhow::Result<()> {
    assets_build::write_files_and_manifest()?;
    Ok(())
}
