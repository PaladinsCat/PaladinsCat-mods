# Performance mod v0.1.0

## Compatibility

- Windows x64; Paladins Season 8.1 Win64, original executable SHA-256 `0c51d381dfe9ecbc1f571ab36603478626375704f4e4e40482524a1adafd6f75`.
- The installed Tempest Launcher CLI. This repository has no build or runtime dependency on a sibling Tempest source checkout.
- Exact `System32/dinput8.dll` SHA-256 `b0f10001fccd05e9863d84588be9b9f8389656d95177287dd4c6cf3be4bf1dfe` and `System32/user32.dll` SHA-256 `f285937030bcb325cad39ee5be4818f6666241229adeadea4ff1ae469dfd7ef3`.

These narrow pins match the build used for native reverse engineering and startup validation. A different executable or Windows update can make this package refuse startup. Do not bypass those checks or substitute an executable to satisfy the launcher.

## Install and launch

1. Download `PaladinsCat-performance-v0.1.0-windows-x64.zip` and the outer `SHA256SUMS.txt` from the v0.1.0 release. Verify the ZIP SHA-256 before extracting.
2. Extract into a writable directory whose path has no comma. Keep the three DLLs and `paladinscat-mods.exe` together.
3. Exit any running Paladins client yourself. The launcher never closes an existing client or attaches to it.
4. Run a read-only preflight, replacing the game path if needed:

```powershell
.\paladinscat-mods.exe --dry-run --game-root "C:\Paladins\Season 8.1" --language INT
```

5. To launch with the compact input mod and world scheduler:

```powershell
.\paladinscat-mods.exe --launch --game-root "C:\Paladins\Season 8.1" --language INT
```

Use `--language KOR` for Korean. The game uses `paladinslive` as its homedir. The default Tempest CLI path is `C:\Program Files\Tempest Launcher\tempest-cli.exe`; override it with `--tempest-cli "ABSOLUTE_PATH"`.

The Rust launcher verifies game, native-module and selected mod hashes, resolves DLLs beside itself, and passes `--dll` to Tempest **before** its `--` game-argument separator. Do not put `--dll` into an instance's native game arguments. A dispatch message proves only that Tempest was started; inspect its saved `tempest-launch-*.log` and the game launch/module state if startup fails.

Existing game graphics and frame settings are retained. This mod installs no FPS limiter and the launcher does not change smoothing, VSync or the user's GPU selection. Disable any existing engine smoothing/cap in your own game configuration when evaluating uncapped performance. The tested maintainer session had native smoothing disabled; a stored maximum of 600 was inactive.

## Included behavior

- Bounded native message-pump work and IME popup housekeeping reuse within a consistent pump.
- One fresh queued-key snapshot for each six-query native modifier sequence.
- Guarded, bounded `GetRawInputBuffer` reads on the existing DirectInput worker; reports go through the original native dispatcher.
- Threshold-four inline scheduling for small native actor/component world-job groups, preserving native job bodies and completion.

No additional input thread, accumulation timer, motion merging or raw-device registration takeover is introduced. Original native message delivery and unfiltered queue handling remain enabled. These structural properties do not establish physical input latency or complete gameplay compatibility.

## Validation and known limits

The activated DLLs passed scoped Rust tests, wrong-process refusal and bridge/ownership checks. Startup validation confirmed both mod DLLs, their native hooks, input batch activity without initial reported errors, retained world-job identities and disabled engine smoothing. The maintainer's game graphics context was verified on an RTX 5080; CPU-only package checks do not verify another machine's selected GPU.

Additional recordings were explicitly skipped for this release. No matched v0.1.0 before/after benchmark establishes an FPS gain, reduced latency or the 1,000 FPS target. Earlier measurements used other input candidates and changing background load or polling rates; they are diagnostic history, not evidence for this release.

Buffered queue removal changes timing and can reorder reports relative to control messages arriving during a read. Malformed reports detected after removal cannot be recovered. Guard overhead can outweigh savings when batches mostly contain one report. Keyboard, buttons, wheel, modifiers, chat/IME, menu and focus behavior still require representative manual validation.

## Roll back

Exit the game manually, then verify and launch the retained raw-read input trial with the same world scheduler:

```powershell
.\paladinscat-mods.exe --dry-run --rollback --language INT
.\paladinscat-mods.exe --launch --rollback --language INT
```

This is a rollback to the previous **modified** input trial, not the unmodified game. To restore the unmodified game, exit it, remove these DLLs from your custom launch configuration, and use your usual normal launch route. The mod does not overwrite the executable, system DLLs or game files. Never unload a running hook DLL; exit the process before changing the mod selection.
