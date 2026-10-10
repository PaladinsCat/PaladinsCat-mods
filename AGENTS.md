# PaladinsCat mods release repository

This is an independent public release checkout under the non-Git PaladinsCat workspace parent. Follow its `AGENTS.md` and `PUBLICATION_BOUNDARY.md`. Keep source implementation of game hooks in WatchCat; publish only explicitly approved immutable binaries as GitHub release assets. Never copy private recordings, diagnostics, credentials, game executables, system DLLs or Tempest binaries into this repository.

Tooling and runtime logic must be Rust. Test with the commands in CONTRIBUTING.md. Preserve game/native/DLL pins, correct Tempest `--dll` placement and manual existing-game exit. No task approval authorizes Computer Use, game launch, restart or capture unless the human separately grants those actions. CPU-only tooling does not prove live GPU routing.

No FPS limiter is introduced; 1000 FPS is a target. v0.1.0 recordings were explicitly skipped by the maintainer. Do not claim verified FPS gain, physical input latency or full compatibility from source tests or startup activation.

Use owner-reviewed pull requests after repository bootstrap. Keep generated release assets under ignored `dist/`; never turn the workspace parent or `local/` into a Git repository.
