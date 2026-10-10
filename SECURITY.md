# Security

Report a suspected security problem privately through this repository's GitHub security advisory reporting, or the PaladinsCat organization owner. Do not publish credentials, private traces or exploit details in a public issue.

The launcher fails closed on changed game, selected input/world DLL or native Windows-module hashes. Do not weaken these pins to support an unknown executable. DLL injection occurs only at an explicit `--launch` through the installed Tempest CLI; the launcher does not attach to a running process or close one.

New versions are distributed through this repository's GitHub Releases. Verify release checksums before use. The package has no automatic updater, watchdog or background service.
