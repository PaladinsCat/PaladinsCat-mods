use paladinscat_mods::{
    Asset, MAX_ASSET_BYTES, PAYLOADS, Recipe, Result, VERSION, digest, read_local_file,
    verify_asset_bytes,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
fn u16le(output: &mut Vec<u8>, value: u16) {
    output.extend(value.to_le_bytes());
}
fn u32le(output: &mut Vec<u8>, value: u32) {
    output.extend(value.to_le_bytes());
}

// ZIP store only: bounded release files, no compression dependency or extraction.
fn zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut central = Vec::new();
    for (name, bytes) in files {
        if name.contains(['/', '\\'])
            || name.contains("..")
            || !name.is_ascii()
            || name.len() > 256
            || bytes.len() > 16 * 1024 * 1024
        {
            return Err("Invalid release ZIP entry".into());
        }
        let offset = u32::try_from(output.len())?;
        let length = u32::try_from(bytes.len())?;
        let name_len = u16::try_from(name.len())?;
        let crc = crc32(bytes);
        u32le(&mut output, 0x04034b50);
        u16le(&mut output, 20);
        u16le(&mut output, 0);
        u16le(&mut output, 0);
        u16le(&mut output, 0);
        u16le(&mut output, 33);
        u32le(&mut output, crc);
        u32le(&mut output, length);
        u32le(&mut output, length);
        u16le(&mut output, name_len);
        u16le(&mut output, 0);
        output.extend(name.as_bytes());
        output.extend(bytes);
        u32le(&mut central, 0x02014b50);
        u16le(&mut central, 20);
        u16le(&mut central, 20);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 33);
        u32le(&mut central, crc);
        u32le(&mut central, length);
        u32le(&mut central, length);
        u16le(&mut central, name_len);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u32le(&mut central, 0);
        u32le(&mut central, offset);
        central.extend(name.as_bytes());
    }
    let offset = u32::try_from(output.len())?;
    let length = u32::try_from(central.len())?;
    output.extend(central);
    u32le(&mut output, 0x06054b50);
    u16le(&mut output, 0);
    u16le(&mut output, 0);
    let count = u16::try_from(files.len())?;
    u16le(&mut output, count);
    u16le(&mut output, count);
    u32le(&mut output, length);
    u32le(&mut output, offset);
    u16le(&mut output, 0);
    Ok(output)
}
fn save_new(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 && args.len() != 5 {
        return Err(
            "Usage: package-release PAYLOAD_DIR LAUNCHER_EXE OUTPUT_DIR SOURCE_REVISION [WORLD_DLL]".into(),
        );
    }
    let payload = PathBuf::from(&args[0]);
    let launcher = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let revision = &args[3];
    if revision.len() != 40 || !revision.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Exact 40-character source revision required".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let recipe = Recipe::parse(&read_local_file(&root, "release-recipe.json", 64 * 1024)?)?;
    if recipe.version != VERSION {
        return Err("Release recipe version differs from the package tool identity".into());
    }
    let version = recipe.version.clone();
    let mut files = Vec::new();
    for asset in &recipe.payloads {
        let bytes = if asset.name == PAYLOADS[1] && args.len() == 5 {
            let world = PathBuf::from(&args[4]).canonicalize()?;
            read_local_file(
                world.parent().ok_or("World payload parent missing")?,
                world
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or("World payload name missing")?,
                MAX_ASSET_BYTES,
            )?
        } else {
            read_local_file(&payload, &asset.name, MAX_ASSET_BYTES)?
        };
        verify_asset_bytes(asset, &bytes)?;
        files.push((asset.name.clone(), bytes));
    }
    let launcher = launcher.canonicalize()?;
    let launcher_root = launcher.parent().ok_or("Launcher parent missing")?;
    let launcher_name = launcher
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or("Launcher filename missing")?;
    files.push((
        "paladinscat-mods.exe".into(),
        read_local_file(launcher_root, launcher_name, MAX_ASSET_BYTES)?,
    ));
    for (name, relative) in [
        ("README.md", "docs/performance-mod.md".to_string()),
        ("RELEASE_NOTES.md", format!("releases/v{version}.md")),
        ("LICENSE", "LICENSE".to_string()),
        ("BINARY-LICENSE.txt", "BINARY-LICENSE.txt".to_string()),
    ] {
        files.push((name.into(), fs::read(root.join(relative))?));
    }
    let assets: Vec<Asset> = files
        .iter()
        .map(|(name, bytes)| Asset {
            name: name.clone(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            bytes: bytes.len() as u64,
        })
        .collect();
    let package = recipe.manifest(revision.to_owned(), assets)?;
    let manifest = serde_json::to_vec_pretty(&package)?;
    let mut sums: String = package
        .assets
        .iter()
        .map(|a| format!("{}  {}\n", a.sha256, a.name))
        .collect();
    sums.push_str(&format!("{:x}  manifest.json\n", Sha256::digest(&manifest)));
    files.push(("manifest.json".into(), manifest.clone()));
    files.push(("SHA256SUMS.txt".into(), sums.into_bytes()));
    let archive_name = format!("PaladinsCat-performance-v{version}-windows-x64.zip");
    fs::create_dir_all(&output)?;
    let bundle = output.join("bundle");
    fs::create_dir_all(&bundle)?;
    for (name, bytes) in &files {
        save_new(&bundle.join(name), bytes)?;
    }
    save_new(&output.join(&archive_name), &zip(&files)?)?;
    save_new(&output.join("manifest.json"), &manifest)?;
    let archive_digest = digest(&output.join(&archive_name))?;
    save_new(
        &output.join("SHA256SUMS.txt"),
        format!(
            "{archive_digest}  {archive_name}\n{:x}  manifest.json\n",
            Sha256::digest(&manifest)
        )
        .as_bytes(),
    )?;
    println!(
        "Package verified from pinned payloads: {}\nSHA-256 {archive_digest}",
        output.join(archive_name).display()
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_matches_zip_standard_vector() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
    }
    #[test]
    fn zip_rejects_traversal() {
        assert!(zip(&[("../bad.dll".into(), vec![])]).is_err());
    }
    #[test]
    fn zip_offsets_and_crc_retain_payload() {
        let data = zip(&[("mod.dll".into(), b"opaque DLL bytes".to_vec())]).unwrap();
        assert_eq!(&data[..4], &0x04034b50u32.to_le_bytes());
        assert_eq!(&data[37..53], b"opaque DLL bytes");
        let end = data.len() - 22;
        let central = u32::from_le_bytes(data[end + 16..end + 20].try_into().unwrap()) as usize;
        assert_eq!(&data[central..central + 4], &0x02014b50u32.to_le_bytes());
        assert_eq!(
            u32::from_le_bytes(data[14..18].try_into().unwrap()),
            crc32(b"opaque DLL bytes")
        );
    }
}
