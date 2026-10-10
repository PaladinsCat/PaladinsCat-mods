use paladinscat_mods::{Options, Result, VERSION, parse};
#[cfg(windows)]
use paladinscat_mods::{launch_arguments, preflight};
#[cfg(windows)]
use std::{path::Path, process::Command};

#[cfg(windows)]
fn hidden(program: &Path) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(0x08000000);
        command
    };
    command
}

#[cfg(windows)]
fn run(options: &Options) -> Result<()> {
    use std::{
        fs::OpenOptions,
        process::Stdio,
        time::{SystemTime, UNIX_EPOCH},
    };
    let bundle = std::env::current_exe()?
        .parent()
        .ok_or("Launcher parent missing")?
        .canonicalize()?;
    let system = std::path::PathBuf::from(std::env::var("SystemRoot")?).join("System32");
    preflight(options, &bundle, &system)?;
    let argv = launch_arguments(options, &bundle)?;
    println!(
        "PaladinsCat performance mod v{VERSION}; pinned game/input/world/native files verified."
    );
    println!("Tempest argv: {argv:?}");
    if !options.launch {
        println!("Read-only preflight: no process launched, closed or changed.");
        return Ok(());
    }
    let tasks = hidden(Path::new("tasklist.exe"))
        .args(["/FO", "CSV", "/NH"])
        .output()?;
    let tasks_text = String::from_utf8_lossy(&tasks.stdout).to_ascii_lowercase();
    if !tasks.status.success()
        || tasks_text.contains("\"paladins.exe\"")
        || tasks_text.contains("\"shippingpc-chaosgame.exe\"")
    {
        return Err("A game client is running or process inventory failed. Exit the client manually before launch; no process was closed.".into());
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let log_path = bundle.join(format!("tempest-launch-{stamp}.log"));
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&log_path)?;
    let error_log = log.try_clone()?;
    let child = hidden(&options.tempest)
        .args(argv)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(error_log)
        .spawn()?;
    println!(
        "Tempest dispatched as PID {}; log {}. Dispatch is not verification that the mod loaded.",
        child.id(),
        log_path.display()
    );
    Ok(())
}

#[cfg(not(windows))]
fn run(_: &Options) -> Result<()> {
    Err("The release launcher requires Windows x64".into())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] || args == ["-h"] {
        println!(
            "PaladinsCat mods v{VERSION}\n  --dry-run | --launch\n  [--rollback] [--game-root ABSOLUTE_PATH]\n  [--tempest-cli ABSOLUTE_PATH] [--language INT|KOR|...]\nDLLs are resolved beside this executable. Exit the game manually before --launch.\n--rollback launches the retained raw-read input trial with the same world scheduler.\nRemoving all custom DLL launch settings restores the unmodified game on its next normal launch."
        );
        return Ok(());
    }
    run(&parse(&args)?)
}
