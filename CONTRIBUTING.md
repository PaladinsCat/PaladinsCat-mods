# Contributing

Use a personal fork and a pull request to protected `main`. Changes require a repository owner's review. Never commit generated DLLs, executables, archives, game data, private diagnostics, credentials or runtime files. Release assets are generated locally from reviewed immutable DLLs; this repository does not rebuild the private WatchCat hooks.

Owner-directed agent work stays in the existing `main` checkout without branches, worktrees, forks or pull requests. Validate locally before pushing and retain GitHub CI after push. Preserve main-only remote rules and never use admin bypass. The contributor procedure above does not override this owner workflow.

All release tooling is Rust. Validate with:

```text
cargo fmt --all --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release --bin paladinscat-mods --bin package-release
```

The launcher is Windows x64. Library tests and package checks are CPU-only. A test or successful dry-run does not authorize a game launch, restart, recording or desktop input.

The package tool takes the private payload directory, built launcher path, output directory and exact 40-character public source commit:

```text
package-release PAYLOAD_DIR LAUNCHER_EXE OUTPUT_DIR SOURCE_COMMIT
```

It refuses changed payload hashes and existing output files. Verify archive members against the manifest and remote release downloads against outer checksums before reporting a release complete. Releasing a mod does not establish its performance or input compatibility; keep these claims tied to actual evidence.
