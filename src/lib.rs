use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PAYLOADS: [&str; 3] = [
    "watchcat-native-input-compact-modifier.dll",
    "watchcat-world-scheduler.dll",
    "watchcat-native-input-raw-read-original.dll",
];
pub const FILES: [&str; 8] = [
    PAYLOADS[0],
    PAYLOADS[1],
    PAYLOADS[2],
    "paladinscat-mods.exe",
    "README.md",
    "RELEASE_NOTES.md",
    "LICENSE",
    "BINARY-LICENSE.txt",
];
pub const MAX_ASSET_BYTES: u64 = 16 * 1024 * 1024;
const MAX_METADATA_BYTES: u64 = 64 * 1024;

fn hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn release_version(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 10
                && p.bytes().all(|c| c.is_ascii_digit())
                && (p.len() == 1 || !p.starts_with('0'))
                && p.parse::<u32>().is_ok()
        })
}

fn flat_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name.is_ascii()
        && !name.contains(['/', '\\', ':', ','])
        && !name.contains("..")
        && !name.ends_with(['.', ' '])
        && !name.chars().any(char::is_control)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerPolicy {
    ActiveLogicalProcessors,
}

fn validate_assets(assets: &[Asset], expected: &[&str]) -> Result<()> {
    let mut names = BTreeSet::new();
    for asset in assets {
        if !flat_name(&asset.name)
            || !hex(&asset.sha256, 64)
            || asset.bytes == 0
            || asset.bytes > MAX_ASSET_BYTES
            || !names.insert(asset.name.as_str())
        {
            return Err("Invalid or duplicate package asset".into());
        }
    }
    if names != expected.iter().copied().collect() {
        return Err("Package asset inventory differs from its protocol".into());
    }
    Ok(())
}

fn validate_identity(
    schema: u32,
    version: &str,
    mod_name: &str,
    platform: &str,
    native: [&str; 3],
) -> Result<()> {
    if schema != 1
        || !release_version(version)
        || mod_name != "paladins-performance"
        || platform != "windows-x86_64"
        || native.iter().any(|h| !hex(h, 64))
    {
        return Err("Unsupported package identity or native compatibility metadata".into());
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    #[serde(rename = "mod")]
    pub mod_name: String,
    pub platform: String,
    pub source_revision: String,
    pub game_sha256: String,
    pub dinput8_sha256: String,
    pub user32_sha256: String,
    pub performance_verified: bool,
    pub compatibility_verified: bool,
    pub rendering: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_policy: Option<WorkerPolicy>,
    pub assets: Vec<Asset>,
}

impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() as u64 > MAX_METADATA_BYTES {
            return Err("Package manifest exceeds its size bound".into());
        }
        let manifest: Self = serde_json::from_slice(bytes)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        validate_identity(
            self.schema,
            &self.version,
            &self.mod_name,
            &self.platform,
            [&self.game_sha256, &self.dinput8_sha256, &self.user32_sha256],
        )?;
        if !hex(&self.source_revision, 40)
            || self.rendering.is_empty()
            || self.rendering.len() > 256
            || self.rendering.chars().any(char::is_control)
        {
            return Err("Invalid package source or rendering metadata".into());
        }
        validate_assets(&self.assets, &FILES)
    }
}

/// Reviewed release inputs. Per-release native identities belong here rather
/// than in the package loader or its accepted-version logic.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub schema: u32,
    pub version: String,
    #[serde(rename = "mod")]
    pub mod_name: String,
    pub platform: String,
    pub game_sha256: String,
    pub dinput8_sha256: String,
    pub user32_sha256: String,
    pub performance_verified: bool,
    pub compatibility_verified: bool,
    pub rendering: String,
    #[serde(default)]
    pub worker_policy: Option<WorkerPolicy>,
    pub payloads: Vec<Asset>,
}

impl Recipe {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() as u64 > MAX_METADATA_BYTES {
            return Err("Release recipe exceeds its size bound".into());
        }
        let recipe: Self = serde_json::from_slice(bytes)?;
        validate_identity(
            recipe.schema,
            &recipe.version,
            &recipe.mod_name,
            &recipe.platform,
            [
                &recipe.game_sha256,
                &recipe.dinput8_sha256,
                &recipe.user32_sha256,
            ],
        )?;
        validate_assets(&recipe.payloads, &PAYLOADS)?;
        Ok(recipe)
    }

    pub fn manifest(self, source_revision: String, assets: Vec<Asset>) -> Result<Manifest> {
        let manifest = Manifest {
            schema: self.schema,
            version: self.version,
            mod_name: self.mod_name,
            platform: self.platform,
            source_revision,
            game_sha256: self.game_sha256,
            dinput8_sha256: self.dinput8_sha256,
            user32_sha256: self.user32_sha256,
            performance_verified: self.performance_verified,
            compatibility_verified: self.compatibility_verified,
            rendering: self.rendering,
            worker_policy: self.worker_policy,
            assets,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

#[cfg(windows)]
fn ordinary_file(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.is_file() && metadata.file_attributes() & 0x400 == 0
}

#[cfg(not(windows))]
fn ordinary_file(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink()
}

pub fn read_local_file(root: &Path, name: &str, max: u64) -> Result<Vec<u8>> {
    if !flat_name(name) {
        return Err("Package file must be an ordinary filename".into());
    }
    let root = root.canonicalize()?;
    let path = root.join(name);
    let metadata = fs::symlink_metadata(&path)?;
    if !ordinary_file(&metadata)
        || metadata.len() > max
        || path.canonicalize()?.parent() != Some(root.as_path())
    {
        return Err(
            format!("Package file escapes its owner or exceeds its size bound: {name}").into(),
        );
    }
    let mut bytes = Vec::new();
    File::open(&path)?.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err("Package file grew beyond its size bound".into());
    }
    Ok(bytes)
}

pub fn verify_asset_bytes(asset: &Asset, bytes: &[u8]) -> Result<()> {
    if bytes.len() as u64 != asset.bytes || format!("{:x}", Sha256::digest(bytes)) != asset.sha256 {
        return Err(format!("Package size or SHA-256 mismatch: {}", asset.name).into());
    }
    Ok(())
}

fn checksum_inventory(bytes: &[u8]) -> Result<BTreeMap<String, String>> {
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err("Package checksums exceed their size bound".into());
    }
    let mut entries = BTreeMap::new();
    for line in std::str::from_utf8(bytes)?.lines() {
        let (hash, name) = line.split_once("  ").ok_or("Invalid checksum entry")?;
        if !hex(hash, 64)
            || !flat_name(name)
            || entries.insert(name.to_owned(), hash.to_owned()).is_some()
        {
            return Err("Invalid or duplicate checksum entry".into());
        }
    }
    Ok(entries)
}

pub fn verify_manifest_checksums(manifest: &Manifest, raw: &[u8], sums: &[u8]) -> Result<()> {
    let mut expected: BTreeMap<_, _> = manifest
        .assets
        .iter()
        .map(|a| (a.name.clone(), a.sha256.clone()))
        .collect();
    expected.insert("manifest.json".into(), format!("{:x}", Sha256::digest(raw)));
    if checksum_inventory(sums)? != expected {
        return Err("Package checksums do not bind the complete manifest inventory".into());
    }
    Ok(())
}

/// Checks the extracted release's complete local inventory. This is integrity
/// validation; local checksums alone do not authenticate a GitHub publisher.
pub fn load_manifest(bundle: &Path) -> Result<Manifest> {
    let raw = read_local_file(bundle, "manifest.json", MAX_METADATA_BYTES)?;
    let manifest = Manifest::parse(&raw)?;
    let sums = read_local_file(bundle, "SHA256SUMS.txt", MAX_METADATA_BYTES)?;
    verify_manifest_checksums(&manifest, &raw, &sums)?;
    for asset in &manifest.assets {
        let bytes = read_local_file(bundle, &asset.name, MAX_ASSET_BYTES)?;
        verify_asset_bytes(asset, &bytes)?;
    }
    Ok(manifest)
}

pub fn digest(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut chunk = [0u8; 65536];
    loop {
        let count = file.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        hash.update(&chunk[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn verify(path: &Path, expected: &str) -> Result<()> {
    if digest(path)? != expected {
        return Err(format!("Pinned SHA-256 mismatch: {}", path.display()).into());
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
pub struct Options {
    pub launch: bool,
    pub rollback: bool,
    pub game_root: PathBuf,
    pub tempest: PathBuf,
    pub language: String,
}

pub fn parse(args: &[String]) -> Result<Options> {
    let mut options = Options {
        launch: false,
        rollback: false,
        game_root: PathBuf::from(r"C:\Paladins\Season 8.1"),
        tempest: PathBuf::from(r"C:\Program Files\Tempest Launcher\tempest-cli.exe"),
        language: "INT".into(),
    };
    let mut mode = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--launch" | "--dry-run" => {
                if mode.is_some() {
                    return Err("Specify exactly one of --launch or --dry-run".into());
                }
                mode = Some(args[index] == "--launch");
            }
            "--rollback" => {
                if options.rollback {
                    return Err("Duplicate --rollback".into());
                }
                options.rollback = true;
            }
            flag @ ("--game-root" | "--tempest-cli" | "--language") => {
                index += 1;
                let value = args.get(index).ok_or("Missing option value")?;
                if value.is_empty() || value.starts_with('-') || value.chars().any(char::is_control)
                {
                    return Err("Invalid option value".into());
                }
                match flag {
                    "--game-root" => options.game_root = value.into(),
                    "--tempest-cli" => options.tempest = value.into(),
                    _ => {
                        if value.len() != 3 || !value.bytes().all(|c| c.is_ascii_uppercase()) {
                            return Err("Language must be a three-letter uppercase game locale, e.g. INT or KOR".into());
                        }
                        options.language.clone_from(value);
                    }
                }
            }
            _ => return Err("Unknown option; use --help".into()),
        }
        index += 1;
    }
    options.launch = mode.ok_or("Specify --dry-run or --launch explicitly")?;
    Ok(options)
}

pub fn launch_arguments(options: &Options, bundle: &Path) -> Result<Vec<String>> {
    let input = bundle.join(PAYLOADS[if options.rollback { 2 } else { 0 }]);
    let world = bundle.join(PAYLOADS[1]);
    let paths = [
        input.to_str().ok_or("Non-Unicode DLL path")?,
        world.to_str().ok_or("Non-Unicode DLL path")?,
    ];
    if paths
        .iter()
        .any(|p| p.contains(',') || p.chars().any(char::is_control))
    {
        return Err("Package path cannot contain a comma or control character: Tempest uses comma-separated DLL paths".into());
    }
    Ok(vec![
        "launch".into(),
        options
            .game_root
            .to_str()
            .ok_or("Non-Unicode game path")?
            .into(),
        "--platform".into(),
        "Win64".into(),
        "--homedir".into(),
        "paladinslive".into(),
        "--dll".into(),
        paths.join(","),
        "--".into(),
        "-nosteam".into(),
        "-EACALT".into(),
        "-eac-nop-loaded".into(),
        "-COOKFORDEMO".into(),
        "-seekfreeloadingpcconsole".into(),
        format!("-Language={}", options.language),
    ])
}

#[cfg(windows)]
pub fn active_logical_processors() -> Result<u32> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetActiveProcessorGroupCount() -> u16;
        fn GetActiveProcessorCount(group: u16) -> u32;
    }
    let groups = unsafe { GetActiveProcessorGroupCount() };
    if groups == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if groups != 1 {
        return Err("The native worker policy requires a single Windows processor group".into());
    }
    let count = unsafe { GetActiveProcessorCount(u16::MAX) };
    if count == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(count)
}

#[cfg(not(windows))]
pub fn active_logical_processors() -> Result<u32> {
    Err("The native worker policy requires Windows".into())
}

fn set_worker_argument(arguments: &mut Vec<String>, count: u32) -> Result<()> {
    if count == 0 || count > i32::MAX as u32 {
        return Err("Invalid native worker count".into());
    }
    let separators: Vec<_> = arguments
        .iter()
        .enumerate()
        .filter_map(|(index, arg)| (arg == "--").then_some(index))
        .collect();
    let [separator] = separators.as_slice() else {
        return Err("Exactly one native game argument separator is required".into());
    };
    let existing: Vec<_> = arguments
        .iter()
        .enumerate()
        .skip(separator + 1)
        .filter_map(|(index, arg)| {
            arg.trim_start_matches(['-', '/'])
                .split_once('=')
                .is_some_and(|(name, _)| name.eq_ignore_ascii_case("THREADPOOL"))
                .then_some(index)
        })
        .collect();
    let value = format!("THREADPOOL={count}");
    match existing.as_slice() {
        [] => arguments.push(value),
        [index] => arguments[*index] = value,
        _ => return Err("Duplicate native THREADPOOL arguments".into()),
    }
    Ok(())
}

fn launch_arguments_with_cpu_count(
    options: &Options,
    bundle: &Path,
    manifest: &Manifest,
    resolve_count: impl FnOnce() -> Result<u32>,
) -> Result<Vec<String>> {
    manifest.validate()?;
    let mut arguments = launch_arguments(options, bundle)?;
    if manifest.worker_policy == Some(WorkerPolicy::ActiveLogicalProcessors) {
        set_worker_argument(&mut arguments, resolve_count()?)?;
    }
    Ok(arguments)
}

pub fn launch_arguments_for_manifest(
    options: &Options,
    bundle: &Path,
    manifest: &Manifest,
) -> Result<Vec<String>> {
    launch_arguments_with_cpu_count(options, bundle, manifest, active_logical_processors)
}

pub fn preflight(options: &Options, bundle: &Path, system: &Path) -> Result<Manifest> {
    if !options.game_root.is_absolute()
        || !options.tempest.is_absolute()
        || !bundle.is_absolute()
        || !system.is_absolute()
    {
        return Err("Game, Tempest, bundle and Windows system paths must be absolute".into());
    }
    let manifest = load_manifest(bundle)?;
    verify_native(&manifest, options, system)?;
    if !options.tempest.is_file() {
        return Err("Installed Tempest CLI is missing".into());
    }
    launch_arguments(options, bundle)?;
    Ok(manifest)
}

pub fn verify_native(manifest: &Manifest, options: &Options, system: &Path) -> Result<()> {
    let game = digest(&options.game_root.join("Binaries/Win64/Paladins.exe"))?;
    let input = digest(&system.join("dinput8.dll"))?;
    let user = digest(&system.join("user32.dll"))?;
    verify_native_digests(manifest, [&game, &input, &user])
}

fn verify_native_digests(manifest: &Manifest, actual: [&str; 3]) -> Result<()> {
    for ((name, expected), actual) in [
        ("Paladins.exe", manifest.game_sha256.as_str()),
        ("dinput8.dll", manifest.dinput8_sha256.as_str()),
        ("user32.dll", manifest.user32_sha256.as_str()),
    ]
    .into_iter()
    .zip(actual)
    {
        if actual != expected {
            return Err(format!("Selected package native compatibility mismatch: {name}").into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).into()).collect()
    }
    #[test]
    fn no_implicit_launch_or_unrecognized_modes() {
        for values in [
            &[][..],
            &["--launch", "--dry-run"][..],
            &["--restart-pid", "123"][..],
            &["--game-root"][..],
        ] {
            assert!(parse(&args(values)).is_err());
        }
    }
    #[test]
    fn launch_preserves_native_separator_and_portable_paths() {
        let options = parse(&args(&["--dry-run", "--language", "KOR"])).unwrap();
        let argv =
            launch_arguments(&options, Path::new("C:/Mods With Spaces/Performance")).unwrap();
        assert_eq!(argv[6], "--dll");
        assert_eq!(argv[8], "--");
        assert!(argv[7].contains("Mods With Spaces"));
        assert!(!argv[7].contains("nabi"));
        assert!(argv[7].contains(PAYLOADS[0]));
        assert!(argv[7].contains(PAYLOADS[1]));
        assert_eq!(argv.last().unwrap(), "-Language=KOR");
    }
    #[test]
    fn rollback_retains_world_and_replaces_input_only() {
        let options = parse(&args(&["--dry-run", "--rollback"])).unwrap();
        let argv = launch_arguments(&options, Path::new("C:/Mods")).unwrap();
        assert!(argv[7].contains(PAYLOADS[2]));
        assert!(argv[7].contains(PAYLOADS[1]));
        assert!(!argv[7].contains(PAYLOADS[0]));
    }
    #[test]
    fn refuses_dll_array_injection_and_bad_locale() {
        let options = parse(&args(&["--dry-run"])).unwrap();
        assert!(launch_arguments(&options, Path::new("C:/Mods,other.dll")).is_err());
        assert!(parse(&args(&["--launch", "--language", "KOR -Other"])).is_err());
    }
    #[test]
    fn pinned_integrity_detects_mismatched_digest() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let expected = digest(&path).unwrap();
        assert!(verify(&path, &expected).is_ok());
        assert!(verify(&path, &"0".repeat(64)).is_err());
    }

    fn fixture(version: &str) -> Manifest {
        Manifest {
            schema: 1,
            version: version.into(),
            mod_name: "paladins-performance".into(),
            platform: "windows-x86_64".into(),
            source_revision: "a".repeat(40),
            game_sha256: "b".repeat(64),
            dinput8_sha256: "c".repeat(64),
            user32_sha256: "d".repeat(64),
            performance_verified: false,
            compatibility_verified: false,
            rendering: "uncapped".into(),
            worker_policy: None,
            assets: FILES
                .iter()
                .map(|name| Asset {
                    name: (*name).into(),
                    sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                    bytes: name.len() as u64,
                })
                .collect(),
        }
    }

    fn sums(manifest: &Manifest, raw: &[u8]) -> Vec<u8> {
        let mut entries: String = manifest
            .assets
            .iter()
            .map(|a| format!("{}  {}\n", a.sha256, a.name))
            .collect();
        entries.push_str(&format!("{:x}  manifest.json\n", Sha256::digest(raw)));
        entries.into_bytes()
    }

    #[test]
    fn future_compatible_versions_use_manifest_native_metadata() {
        for version in [VERSION, "71.23.109"] {
            let manifest = fixture(version);
            let raw = serde_json::to_vec(&manifest).unwrap();
            let loaded = Manifest::parse(&raw).unwrap();
            assert_eq!(loaded.version, version);
            assert!(verify_manifest_checksums(&loaded, &raw, &sums(&loaded, &raw)).is_ok());
            let hashes = [
                &loaded.game_sha256[..],
                &loaded.dinput8_sha256[..],
                &loaded.user32_sha256[..],
            ];
            assert!(verify_native_digests(&loaded, hashes).is_ok());
            for index in 0..3 {
                let mut changed = hashes;
                let other = "e".repeat(64);
                changed[index] = &other;
                assert!(verify_native_digests(&loaded, changed).is_err());
            }
        }
    }

    #[test]
    fn manifest_rejects_identity_inventory_and_native_metadata_tampering() {
        let good = fixture("71.23.109");
        let mut bad = good.clone();
        bad.schema += 1;
        assert!(bad.validate().is_err());
        for version in ["071.23.109", "71.23.109-rc1", "71.23", "71.23.4294967296"] {
            bad = good.clone();
            bad.version = version.into();
            assert!(bad.validate().is_err());
        }
        bad = good.clone();
        bad.mod_name.push('x');
        assert!(bad.validate().is_err());
        bad = good.clone();
        bad.platform.push('x');
        assert!(bad.validate().is_err());
        bad = good.clone();
        bad.source_revision.pop();
        assert!(bad.validate().is_err());
        bad = good.clone();
        bad.game_sha256 = "G".repeat(64);
        assert!(bad.validate().is_err());
        bad = good.clone();
        bad.assets.pop();
        assert!(bad.validate().is_err());
        bad = good.clone();
        bad.assets.push(good.assets[0].clone());
        assert!(bad.validate().is_err());
        for name in [
            "../other.dll",
            "C:other.dll",
            "sub\\other.dll",
            "other.dll",
            "other.dll.",
        ] {
            bad = good.clone();
            bad.assets[0].name = name.into();
            assert!(bad.validate().is_err());
        }
        for bytes in [0, MAX_ASSET_BYTES + 1] {
            bad = good.clone();
            bad.assets[0].bytes = bytes;
            assert!(bad.validate().is_err());
        }
        let raw = serde_json::to_string(&good).unwrap();
        let duplicate = raw.replacen('{', "{\"schema\":1,", 1);
        assert!(Manifest::parse(duplicate.as_bytes()).is_err());
        let unknown = raw.replacen('{', "{\"loader_override\":true,", 1);
        assert!(Manifest::parse(unknown.as_bytes()).is_err());
    }

    #[test]
    fn checksum_inventory_binds_manifest_and_refuses_duplicates_or_paths() {
        let manifest = fixture("71.23.109");
        let raw = serde_json::to_vec(&manifest).unwrap();
        let good = sums(&manifest, &raw);
        assert!(verify_manifest_checksums(&manifest, &raw, &good).is_ok());
        assert!(verify_manifest_checksums(&manifest, b"changed manifest", &good).is_err());
        let mut duplicate = good.clone();
        duplicate.extend(format!("{}  manifest.json\n", "a".repeat(64)).bytes());
        assert!(verify_manifest_checksums(&manifest, &raw, &duplicate).is_err());
        let mut extra = good.clone();
        extra.extend(format!("{}  ../other.dll\n", "a".repeat(64)).bytes());
        assert!(verify_manifest_checksums(&manifest, &raw, &extra).is_err());
        assert!(
            verify_manifest_checksums(
                &manifest,
                &raw,
                good.splitn(2, |b| *b == b'\n').nth(1).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn payload_hash_and_length_are_both_required() {
        let bytes = b"native payload";
        let asset = Asset {
            name: PAYLOADS[1].into(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            bytes: bytes.len() as u64,
        };
        assert!(verify_asset_bytes(&asset, bytes).is_ok());
        assert!(verify_asset_bytes(&asset, b"Native payload").is_err());
        assert!(verify_asset_bytes(&asset, &bytes[..bytes.len() - 1]).is_err());
        for name in ["../Cargo.toml", "src/lib.rs", "C:Cargo.toml"] {
            assert!(
                read_local_file(Path::new(env!("CARGO_MANIFEST_DIR")), name, MAX_ASSET_BYTES)
                    .is_err()
            );
        }
        assert!(read_local_file(Path::new(env!("CARGO_MANIFEST_DIR")), "Cargo.toml", 1).is_err());
    }

    #[test]
    fn recipe_requires_complete_payload_inventory() {
        let raw = include_bytes!("../release-recipe.json");
        assert!(Recipe::parse(raw).is_ok());
        let mut recipe: serde_json::Value = serde_json::from_slice(raw).unwrap();
        recipe["version"] = "71.23.109".into();
        assert!(Recipe::parse(&serde_json::to_vec(&recipe).unwrap()).is_ok());
        recipe["payloads"].as_array_mut().unwrap().pop();
        assert!(Recipe::parse(&serde_json::to_vec(&recipe).unwrap()).is_err());
    }

    #[test]
    fn legacy_manifest_keeps_arguments_without_querying_topology() {
        let options = parse(&args(&["--dry-run", "--rollback"])).unwrap();
        let bundle = Path::new("C:/Mods");
        let manifest = fixture("0.1.2");
        let raw = serde_json::to_vec(&manifest).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("worker_policy"));
        let loaded = Manifest::parse(&raw).unwrap();
        assert_eq!(loaded.worker_policy, None);
        let actual = launch_arguments_with_cpu_count(&options, bundle, &loaded, || {
            panic!("Legacy metadata must not select a worker policy")
        })
        .unwrap();
        assert_eq!(actual, launch_arguments(&options, bundle).unwrap());
    }

    #[test]
    fn selected_manifest_policy_sets_dynamic_native_count_for_both_input_variants() {
        for rollback in [false, true] {
            let mut options = parse(&args(&["--dry-run"])).unwrap();
            options.rollback = rollback;
            let bundle = Path::new("C:/Mods With Spaces");
            let mut manifest = fixture("71.23.109");
            manifest.worker_policy = Some(WorkerPolicy::ActiveLogicalProcessors);
            let baseline = launch_arguments(&options, bundle).unwrap();
            for count in [1, 13, 57] {
                let mut queries = 0;
                let actual = launch_arguments_with_cpu_count(&options, bundle, &manifest, || {
                    queries += 1;
                    Ok(count)
                })
                .unwrap();
                assert_eq!(queries, 1);
                assert_eq!(&actual[..baseline.len()], baseline);
                assert_eq!(actual[6], "--dll");
                assert_eq!(actual[8], "--");
                assert_eq!(actual.last().unwrap(), &format!("THREADPOOL={count}"));
                assert_eq!(
                    actual
                        .iter()
                        .filter(|a| a.starts_with("THREADPOOL="))
                        .count(),
                    1
                );
                assert_eq!(manifest.version, "71.23.109");
                assert_eq!(manifest.source_revision, "a".repeat(40));
            }
        }
    }

    #[test]
    fn worker_policy_rejects_unknown_values_and_survives_recipe_packaging() {
        let fixture = fixture("71.23.109");
        let mut manifest = serde_json::to_value(&fixture).unwrap();
        let mut recipe: serde_json::Value =
            serde_json::from_slice(include_bytes!("../release-recipe.json")).unwrap();
        recipe["version"] = "71.23.109".into();
        for value in [
            serde_json::json!("fixed_eight"),
            serde_json::json!("ActiveLogicalProcessors"),
            serde_json::json!(17),
            serde_json::json!({"active_logical_processors": true}),
        ] {
            manifest["worker_policy"] = value.clone();
            recipe["worker_policy"] = value;
            assert!(Manifest::parse(&serde_json::to_vec(&manifest).unwrap()).is_err());
            assert!(Recipe::parse(&serde_json::to_vec(&recipe).unwrap()).is_err());
        }
        recipe["worker_policy"] = "active_logical_processors".into();
        let recipe = Recipe::parse(&serde_json::to_vec(&recipe).unwrap()).unwrap();
        let packaged = recipe
            .manifest(fixture.source_revision.clone(), fixture.assets)
            .unwrap();
        let raw = serde_json::to_vec(&packaged).unwrap();
        assert_eq!(
            Manifest::parse(&raw).unwrap().worker_policy,
            Some(WorkerPolicy::ActiveLogicalProcessors)
        );
        assert_eq!(packaged.version, "71.23.109");
        let duplicate = String::from_utf8(raw).unwrap().replacen(
            '{',
            "{\"worker_policy\":\"active_logical_processors\",",
            1,
        );
        assert!(Manifest::parse(duplicate.as_bytes()).is_err());
    }

    #[test]
    fn worker_argument_replaces_one_value_and_refuses_duplicates_without_changes() {
        let mut arguments = args(&["--dll", "C:/THREADPOOL=payload.dll", "--", "-ThreadPool=8"]);
        set_worker_argument(&mut arguments, 23).unwrap();
        assert_eq!(arguments[1], "C:/THREADPOOL=payload.dll");
        assert_eq!(arguments[3], "THREADPOOL=23");
        arguments.push("/threadpool=9".into());
        let duplicate = arguments.clone();
        assert!(set_worker_argument(&mut arguments, 31).is_err());
        assert_eq!(arguments, duplicate);
        for count in [0, u32::MAX] {
            let mut arguments = args(&["--", "-nosteam"]);
            let previous = arguments.clone();
            assert!(set_worker_argument(&mut arguments, count).is_err());
            assert_eq!(arguments, previous);
        }
        for mut arguments in [args(&["launch"]), args(&["--", "--"])] {
            assert!(set_worker_argument(&mut arguments, 7).is_err());
        }
    }

    #[test]
    fn worker_policy_validates_package_source_and_propagates_cpu_query_failure() {
        let options = parse(&args(&["--dry-run"])).unwrap();
        let mut manifest = fixture("71.23.109");
        manifest.worker_policy = Some(WorkerPolicy::ActiveLogicalProcessors);
        assert!(
            launch_arguments_with_cpu_count(&options, Path::new("C:/Mods"), &manifest, || {
                Err("unsupported processor topology".into())
            })
            .is_err()
        );
        manifest.source_revision.clear();
        assert!(
            launch_arguments_with_cpu_count(&options, Path::new("C:/Mods"), &manifest, || {
                panic!("Invalid package provenance must be rejected before topology lookup")
            })
            .is_err()
        );
    }

    #[test]
    fn loader_has_no_compiled_release_versions_or_fingerprints() {
        let source = include_str!("lib.rs").split("#[cfg(test)]").next().unwrap();
        for literal in source.split('"').skip(1).step_by(2) {
            assert!(!hex(literal, 64), "Compiled fingerprint: {literal}");
            assert!(
                !release_version(literal),
                "Compiled release selector: {literal}"
            );
        }
    }
}
