//! Chat font coverage: bundled faces, Windows script fallbacks and user fonts.
//!
//! Chat comments arrive in whatever script viewers write in. The bundled
//! Inter/JetBrains faces only cover Latin-ish glyphs, so Japanese, Korean,
//! Chinese, Arabic, Thai and many other scripts used to render as empty
//! boxes (tofu). egui resolves missing glyphs by walking the font list of a
//! family in order, so the fix is a longer, ordered fallback chain:
//!
//! 1. The bundled primary face (Inter / JetBrains Mono).
//! 2. **User fonts** dropped into `%APPDATA%\TikTokStreamTool\fonts`
//!    (`.ttf`, `.otf`, `.ttc`, `.otc`). These win over the system faces, so a
//!    viewer can force a favourite look for any script.
//! 3. Well-known **Windows system fonts** covering CJK, Korean, Arabic,
//!    Indic, Thai and symbols. Only the ones actually installed are loaded.
//! 4. The bundled monochrome Noto Emoji face, last, exactly as before.
//!
//! Everything is loaded lazily enough that a missing file is skipped, never
//! fatal. Use [`install`] at start-up and [`reload`] after the user adds or
//! removes font files.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

use crate::persistence::paths;

/// What the last font scan produced, shown on the settings page.
#[derive(Default)]
pub struct FontReport {
    /// Directory scanned for user fonts (may not exist yet).
    pub custom_dir: Option<PathBuf>,
    /// User font files registered, most specific first.
    pub custom: Vec<String>,
    /// System fallback faces registered (display names).
    pub system: Vec<String>,
    /// Files that existed but could not be read.
    pub failed: Vec<String>,
}

/// The folder users drop custom `.ttf`/`.otf` files into.
pub fn custom_font_dir() -> Option<PathBuf> {
    paths::config_dir().map(|dir| dir.join("fonts"))
}

/// Install the full font chain on the context, synchronously. Used by the
/// settings "Reload fonts" button.
pub fn install(ctx: &egui::Context) -> FontReport {
    let (fonts, report) = build();
    ctx.set_fonts(fonts);
    report
}

/// Fast start-up path: install **only the bundled faces** (no disk I/O), so
/// the first window frame is not blocked reading ~70 MB of system fonts.
/// The extended chain arrives moments later via [`spawn_extended`].
pub fn install_bundled(ctx: &egui::Context) {
    let mut fonts = bundled();
    order_families(&mut fonts, &[], &[]);
    ctx.set_fonts(fonts);
}

/// Load custom + system fonts on a background thread and install the full
/// chain when ready. Returns a channel that yields the scan report exactly
/// once; the UI should poll it and store the report for the settings page.
pub fn spawn_extended(ctx: egui::Context) -> std::sync::mpsc::Receiver<FontReport> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("font-load".to_string())
        .spawn(move || {
            let (fonts, report) = build();
            // `set_fonts` only queues new definitions (lazy rebuild), and the
            // repaint makes the UI pick them up immediately.
            ctx.set_fonts(fonts);
            ctx.request_repaint();
            let _ = tx.send(report);
        });
    rx
}

/// Re-scan user + system fonts and rebuild the chain. Existing text re-layouts
/// automatically because `Context::set_fonts` clears the glyph caches.
pub fn reload(ctx: &egui::Context) -> FontReport {
    install(ctx)
}

/// `(FontDefinitions, FontReport)` for the current machine.
fn build() -> (FontDefinitions, FontReport) {
    let mut fonts = bundled();
    let mut report = FontReport {
        custom_dir: custom_font_dir(),
        ..FontReport::default()
    };

    let mut custom_ids = Vec::new();
    if let Some(dir) = report.custom_dir.clone() {
        for path in scan_font_files(&dir) {
            let name = file_label(&path);
            match std::fs::read(&path) {
                Ok(bytes) if !bytes.is_empty() => {
                    let id = format!("custom/{name}");
                    fonts
                        .font_data
                        .insert(id.clone(), Arc::new(FontData::from_owned(bytes)));
                    custom_ids.push(id);
                    report.custom.push(name);
                }
                _ => report.failed.push(name),
            }
        }
    }

    let mut system_ids = Vec::new();
    for candidate in SYSTEM_CANDIDATES {
        let path = Path::new(r"C:\Windows\Fonts").join(candidate.file);
        if !path.is_file() {
            continue;
        }
        match std::fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => {
                let id = format!("system/{}", candidate.file);
                fonts
                    .font_data
                    .insert(id.clone(), Arc::new(FontData::from_owned(bytes)));
                system_ids.push(id);
                report.system.push(candidate.name.to_owned());
            }
            _ => report.failed.push(candidate.file.to_owned()),
        }
    }

    order_families(&mut fonts, &custom_ids, &system_ids);

    (fonts, report)
}

/// Order matters: primary face, user fonts, system fallbacks, the egui
/// defaults that shipped with FontDefinitions, then monochrome emoji.
fn order_families(fonts: &mut FontDefinitions, custom_ids: &[String], system_ids: &[String]) {
    for (family, primary) in [
        (FontFamily::Proportional, "inter"),
        (FontFamily::Monospace, "jetbrains"),
    ] {
        let defaults = fonts.families.get(&family).cloned().unwrap_or_default();
        let mut ordered = Vec::new();
        ordered.push(primary.to_owned());
        ordered.extend(custom_ids.iter().cloned());
        ordered.extend(system_ids.iter().cloned());
        ordered.extend(defaults);
        ordered.push("notoemoji".to_owned());
        ordered.dedup();
        fonts.families.insert(family, ordered);
    }
}

/// The faces compiled into the binary (unchanged from the original app).
fn bundled() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter.ttf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/JetBrainsMono.ttf"
        ))),
    );
    // ASCII subset of Zen Antique, used only for the "ShuKanji" wordmark.
    fonts.font_data.insert(
        "zen".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/ZenAntique-Wordmark.ttf"
        ))),
    );
    fonts.font_data.insert(
        "notoemoji".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/NotoEmoji.ttf"
        ))),
    );
    fonts
        .families
        .insert(FontFamily::Name("wordmark".into()), vec!["zen".to_owned()]);
    fonts
}

/// All font files in `dir` (non-recursive), sorted by file name so the user
/// controls priority by renaming (e.g. `01-my-font.ttf`).
fn scan_font_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_font_file(p))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

pub fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "ttf" | "otf" | "ttc" | "otc"
            )
        })
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

struct SystemFont {
    file: &'static str,
    name: &'static str,
}

/// Well-known Windows faces, in fallback priority order. Broad Latin and
/// symbol coverage first, then per-script faces; earlier entries win when two
/// fonts cover the same glyph. `.ttc` collections load their first face.
const SYSTEM_CANDIDATES: &[SystemFont] = &[
    SystemFont {
        file: "segoeui.ttf",
        name: "Segoe UI",
    },
    SystemFont {
        file: "seguisym.ttf",
        name: "Segoe UI Symbol",
    },
    SystemFont {
        file: "YuGothR.ttc",
        name: "Yu Gothic (Japanese)",
    },
    SystemFont {
        file: "meiryo.ttc",
        name: "Meiryo (Japanese)",
    },
    SystemFont {
        file: "msyh.ttc",
        name: "Microsoft YaHei (Chinese Simplified)",
    },
    SystemFont {
        file: "msjh.ttc",
        name: "Microsoft JhengHei (Chinese Traditional)",
    },
    SystemFont {
        file: "malgun.ttf",
        name: "Malgun Gothic (Korean)",
    },
    SystemFont {
        file: "Nirmala.ttc",
        name: "Nirmala UI (Indic)",
    },
    SystemFont {
        file: "LeelawUI.ttf",
        name: "Leelawadee UI (Thai)",
    },
    SystemFont {
        file: "simsun.ttc",
        name: "SimSun (Chinese)",
    },
    SystemFont {
        file: "arial.ttf",
        name: "Arial",
    },
    SystemFont {
        file: "tahoma.ttf",
        name: "Tahoma",
    },
    SystemFont {
        file: "seguiemj.ttf",
        name: "Segoe UI Emoji",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_extension_filter() {
        assert!(is_font_file(Path::new("a.ttf")));
        assert!(is_font_file(Path::new("b.OTF")));
        assert!(is_font_file(Path::new("c.ttc")));
        assert!(!is_font_file(Path::new("readme.txt")));
        assert!(!is_font_file(Path::new("no_extension")));
    }

    #[test]
    fn build_keeps_primary_faces_first_and_emoji_last() {
        let (fonts, report) = build();
        let proportional = fonts
            .families
            .get(&FontFamily::Proportional)
            .expect("proportional family");
        assert_eq!(proportional.first().map(String::as_str), Some("inter"));
        assert_eq!(
            proportional.last().map(String::as_str),
            Some("notoemoji")
        );
        // Custom faces must come before system fallbacks when both exist.
        if !report.custom.is_empty() && !report.system.is_empty() {
            let custom_pos = proportional
                .iter()
                .position(|id| id.starts_with("custom/"))
                .unwrap();
            let system_pos = proportional
                .iter()
                .position(|id| id.starts_with("system/"))
                .unwrap();
            assert!(custom_pos < system_pos);
        }
        let mono = fonts
            .families
            .get(&FontFamily::Monospace)
            .expect("monospace family");
        assert_eq!(mono.first().map(String::as_str), Some("jetbrains"));
    }
}
