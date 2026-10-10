# PaladinsCat mods

Versioned Windows client mod releases from [PaladinsCat](https://github.com/PaladinsCat).

## First release

[Performance mod v0.1.0](https://github.com/PaladinsCat/PaladinsCat-mods/releases/tag/v0.1.0) contains native input processing improvements and a small world-job scheduler change for the pinned Paladins Season 8.1 Win64 client.

Download the Windows x64 ZIP from Releases, extract it into a writable directory, and follow the [installation and rollback guide](docs/performance-mod.md). Game and Windows module hashes must match; this release deliberately refuses other builds.

The mod adds no FPS limiter. **1,000 FPS is a performance target, not a cap or a guaranteed result.** Startup activation was verified. Further benchmark recordings were skipped at the maintainer's request, so v0.1.0 has no verified performance gain or complete input compatibility result.

## Repository layout

- `src/`: public Rust launcher and package tooling.
- `docs/`: installation, compatibility and rollback instructions.
- `mods/`: release catalog and immutable DLL identities.
- `releases/`: release notes.
- `.github/`: CI, CodeQL and ownership rules.

Compiled mod DLLs are published as release assets. Their implementation remains maintained in the separate WatchCat project. This repository contains no game binaries, operating-system DLLs, Tempest binaries, recordings, credentials or private diagnostics.

For validation and contribution rules, see [CONTRIBUTING.md](CONTRIBUTING.md). Public release tooling is MIT licensed; mod binaries have separate [distribution terms](BINARY-LICENSE.txt).
