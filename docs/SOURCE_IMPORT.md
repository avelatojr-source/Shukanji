# Import the complete source tree

## Current GitHub status

The repository currently contains the project README, ignore rules, license files, engineering documentation, and the source-import helper. The Rust application source, TikTok LIVE client source, and embedded binary assets are not yet expanded into the GitHub tree. The repository is **not buildable** until both crates and all embedded assets are present.

## Why a local import is needed

The organized source archive contains several megabytes of source and binary assets (fonts, Windows icons, and mascot WebP files). The connected GitHub API can edit text files, but it cannot transfer this complete binary archive or run `git push` from your Windows checkout. The local Git client can transfer the exact archive bytes without converting or truncating them.

## Import using the PowerShell helper

1. Download `ShuKanji-Organized-Source.zip` from the file link supplied with this handoff and save it locally, for example at `$HOME\Downloads\ShuKanji-Organized-Source.zip`.
2. Clone this repository:

   ```powershell
   git clone https://github.com/avelatojr-source/Shukanji.git
   cd Shukanji
   ```

3. Run the checked-in helper:

   ```powershell
   powershell -ExecutionPolicy Bypass -File .\scripts\import-organized-source.ps1 -ArchivePath "$HOME\Downloads\ShuKanji-Organized-Source.zip"
   ```

4. Review the staged file list. The script asks before it commits and pushes. Answer `y` to publish the source.

The helper copies only `app/` and `live-rs/`; it preserves this repository's top-level README and docs. It validates the required Cargo files, the `../live-rs` path dependency, four font files, the Windows icon and the three mascot WebP files. It refuses archives containing nested `.git`, `target/`, `dist/`, `Source code/`, `config.toml`, or `.exe` files. It also requires a clean `main` checkout up to date with `origin/main`. It never force-pushes.

## Required layout

Keep the two crates as siblings:

```text
Shukanji/
├── app/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── build.rs
│   ├── assets/
│   └── src/
└── live-rs/
    ├── Cargo.toml
    ├── src/
    └── tests/
```

Do not put `live-rs` inside `app/`, and do not create nested Git repositories. The desktop crate intentionally uses `path = "../live-rs"`.

## After pushing

On Windows, verify the build from `app/` and run the `live-rs` tests separately. See [RELEASING.md](RELEASING.md) and [MANUAL_OBS_CHECKLIST.md](MANUAL_OBS_CHECKLIST.md). Do not mark a release build-ready until these checks have passed on Windows.
