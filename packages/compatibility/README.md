# `@dshplusplus/compatibility`

Reads the DSH++ runtime manifest and reports whether the current Node runtime and an optional DeepSeek Harness checkout match the pinned M0 baseline.

```powershell
pnpm doctor -- --dsh-root D:\DeepSeekHarness
pnpm doctor -- --dsh-root D:\DeepSeekHarness --json
```

The command is diagnostic only. It never installs dependencies, edits DSH, updates the manifest, or reads credentials.

## Known Limitations and Deferred Work

- M0 checks Node, DSH source version, and commit. Published-package closure, session format, SQLite schema, MCA protocol, browser runtime, signatures, and file hashes are added as their artifacts exist.

