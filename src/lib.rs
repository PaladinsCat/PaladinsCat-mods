use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GAME_SHA: &str = "0c51d381dfe9ecbc1f571ab36603478626375704f4e4e40482524a1adafd6f75";
pub const DINPUT_SHA: &str = "b0f10001fccd05e9863d84588be9b9f8389656d95177287dd4c6cf3be4bf1dfe";
pub const USER32_SHA: &str = "f285937030bcb325cad39ee5be4818f6666241229adeadea4ff1ae469dfd7ef3";
pub const PAYLOADS: [(&str, &str); 3] = [
    (
        "watchcat-native-input-compact-modifier.dll",
        "f45578bc07a204dd3b992a5faca7945ca1695b8a9386f4ac0072ec9b463a3f1d",
    ),
    (
        "watchcat-world-scheduler.dll",
        "2b5893da71714f36548146d466f310490f859354803a0d0a71123bc53464e0e4",
    ),
    (
        "watchcat-native-input-raw-read-original.dll",
        "2da0b9da101ddf82bbc7e2f3a61205b47e96bd9200864bb86854d4fb17de61e6",
    ),
];

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
    let input = bundle.join(PAYLOADS[if options.rollback { 2 } else { 0 }].0);
    let world = bundle.join(PAYLOADS[1].0);
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

pub fn preflight(options: &Options, bundle: &Path, system: &Path) -> Result<()> {
    if !options.game_root.is_absolute()
        || !options.tempest.is_absolute()
        || !bundle.is_absolute()
        || !system.is_absolute()
    {
        return Err("Game, Tempest, bundle and Windows system paths must be absolute".into());
    }
    verify(
        &options.game_root.join("Binaries/Win64/Paladins.exe"),
        GAME_SHA,
    )?;
    verify(
        &bundle.join(PAYLOADS[if options.rollback { 2 } else { 0 }].0),
        PAYLOADS[if options.rollback { 2 } else { 0 }].1,
    )?;
    verify(&bundle.join(PAYLOADS[1].0), PAYLOADS[1].1)?;
    verify(&system.join("dinput8.dll"), DINPUT_SHA)?;
    verify(&system.join("user32.dll"), USER32_SHA)?;
    if !options.tempest.is_file() {
        return Err("Installed Tempest CLI is missing".into());
    }
    launch_arguments(options, bundle)?;
    Ok(())
}

#[derive(Serialize)]
pub struct Asset {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
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
        assert!(argv[7].contains(PAYLOADS[0].0));
        assert!(argv[7].contains(PAYLOADS[1].0));
        assert_eq!(argv.last().unwrap(), "-Language=KOR");
    }
    #[test]
    fn rollback_retains_world_and_replaces_input_only() {
        let options = parse(&args(&["--dry-run", "--rollback"])).unwrap();
        let argv = launch_arguments(&options, Path::new("C:/Mods")).unwrap();
        assert!(argv[7].contains(PAYLOADS[2].0));
        assert!(argv[7].contains(PAYLOADS[1].0));
        assert!(!argv[7].contains(PAYLOADS[0].0));
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
}
