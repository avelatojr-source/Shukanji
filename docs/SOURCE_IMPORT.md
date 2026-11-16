# Import the complete source tree

## Current GitHub status

The repository currently has the project README, ignore rules, license files, and engineering documentation. The Rust application source and bundled assets are not yet expanded into the GitHub tree. The repository is not buildable until both crates and all embedded assets are present.

## Why a local import is needed

The complete source archive is several megabytes and includes binary font, icon, and mascot assets. Use the local Git client to copy those exact bytes into the repository; the helper below checks the archive and preserves the sibling-crate layout.

## Import using the PowerShell helper

1. Download the [organized source archive](https://github.com/avelatojr-source/Shukanji) from the project handoff provided with this repository update.
2. Clone this repository:
   ```powershell
   git clone https://github.com/avelatojr-source/Shukanji.git
   cd Shukanji
   ```
3. Save the organized source ZIP locally, for example at `$HOME\Downloads\ShuKanji-Organized-Source.zip`.
4. Run:
   ```powershell
   powershell -ExecutionPolicy Bypass -File .\scripts\import-organized-source.ps1 -ArchivePath "$HOME\Downloads\ShuKanji-Organized-Source.zip"
   ```
5. Review the staged file list. The script asks before it commits and pushes.

The helper copies only `app/` and `live-rs/`; it preserves this repository's top-level README and docs. It validates the important Cargo files and `../live-rs` path dependency, and refuses archives containing nested `.git`, `target/`, `dist/`, `Source code/`, `config.toml`, or `.exe` files. It never force-pushes.

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
