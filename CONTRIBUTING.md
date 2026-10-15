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
package-release PAYLOAD_DIR LAUNCHER_EXE OUTPUT_DIR SOURCE_COMMIT [WORLD_DLL]
```

`release-recipe.json` owns the release version, approved native payload names,
SHA-256 digests and byte lengths, and game/Windows compatibility fingerprints.
Update that reviewed metadata when approved payloads change; never put release
hashes, compatibility fingerprints or accepted-version lists in loader code.
The tool verifies the recipe's complete payload inventory and emits the same
schema-1 manifest consumed by the public launcher. Its own Cargo version must
match the recipe for release identity.
The optional world-DLL input permits a newly built world payload alongside
unchanged input payloads from a retained bundle. Its recipe hash and size remain
mandatory; the earlier bundle is never modified.

The standalone wrapper reads `manifest.json` and `SHA256SUMS.txt` beside its
executable. It verifies the source revision format, complete manifest/checksum
inventory, ordinary local files, all sizes and hashes, and selected package's
native compatibility before either dry-run or launch. It accepts future
compatible manifest versions without a source change. Local checksums establish
integrity only; obtain the package from the canonical GitHub release and verify
its outer archive checksum to establish the delivery source.

The package tool refuses changed payloads and existing output files. Verify
archive members against the manifest and remote release downloads against outer
checksums before reporting a release complete. Releasing a mod does not establish
its performance or input compatibility; keep these claims tied to actual evidence.
