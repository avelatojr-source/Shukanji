# Source import status

The project source archive was reorganized into a repository layout with sibling `app/` and `live-rs/` Rust crates, plus project documentation.

This GitHub repository currently contains the project overview, repository ignore rules, licenses, and engineering documentation. The full application source and embedded binary assets have **not** yet been uploaded to this repository.

The app embeds binary font files, mascot images, and Windows application icons at compile time. Those files are part of the source and must be committed with the Rust code; omitting them would leave the repository incomplete and may prevent the application from compiling.

## Complete the import from the organized source archive

1. Download and extract `ShuKanji-Organized-Source.zip`.
2. Copy the contents of the extracted `ShuKanji-Organized/` directory into the root of this repository.
3. Keep `app/` and `live-rs/` as sibling directories. Do not turn the repository into a nested Git repository or a Cargo workspace.
4. Confirm `app/Cargo.toml` still points to `../live-rs`.
5. Run the release checklist in [RELEASING.md](RELEASING.md).
6. Check `git status` and confirm no `target/`, `dist/`, `Source code/`, `config.toml`, or executable output is staged.

The repository should only be described as buildable after the full source tree, including its embedded assets, has been pushed and a Windows build has been checked.
