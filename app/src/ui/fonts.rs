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
//! 3. **Windows system fallbacks** covering symbols, CJK, Korean, Arabic,
//!    Indic, Thai and emoji. Only the ones actually installed are loaded.
//! 4. The bundled monochrome Noto Emoji face, last, exactly as before.
//!
//! ## Lazy script fonts (RAM)
//!
//! CJK font collections are large (20 MB each once parsed). Loading every
//! script at start-up cost roughly 200 MB of private memory even for users who
//! only read English chat. [`FontManager`] therefore installs the small base
//! faces immediately and loads the Japanese / Chinese / Korean / Thai / Indic /
//! colour-emoji face only when [`FontManager::ensure_text`] first sees that
//! script. Opening Settings → Chat fonts intentionally triggers every script so
//! the preview is complete.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use egui::{FontData, FontDefinitions, FontFamily};

use crate::persistence::paths;

/// What the last font scan produced, shown on the settings page.
#[derive(Default, Clone)]
pub struct FontReport {
    /// Directory scanned for user fonts (may not exist yet).
    pub custom_dir: Option<PathBuf>,
    /// User font files registered, most specific first.
    pub custom: Vec<String>,
    /// System fallback faces currently installed (display names).
    pub system: Vec<String>,
    /// Files that existed but could not be read.
    pub failed: Vec<String>,
}

/// A script whose (large) fallback font is loaded on demand.
#[derive(Clone, Copy)]
enum Script {
    Japanese,
    Chinese,
    Korean,
    Thai,
    Indic,
    Emoji,
}

impl Script {
    const ALL: [Script; 6] = [
        Script::Japanese,
        Script::Chinese,
        Script::Korean,
        Script::Thai,
        Script::Indic,
        Script::Emoji,
    ];

    fn bit(self) -> u8 {
        1 << (self as u8)
    }

    /// The Windows font file that covers this script.
    fn file(self) -> &'static str {
        match self {
            Script::Japanese => "YuGothR.ttc",
            Script::Chinese => "msyh.ttc",
            Script::Korean => "malgun.ttf",
            Script::Thai => "LeelawUI.ttf",
            Script::Indic => "Nirmala.ttc",
            Script::Emoji => "seguiemj.ttf",
        }
    }
}

/// Faces installed on every start-up. Small, and they cover Latin, symbols,
/// Arabic and Hebrew (Segoe UI) plus the symbol planes.
const BASE_FONTS: &[&str] = &["segoeui.ttf", "seguisym.ttf"];

/// A background font worker request.
enum FontRequest {
    /// Load these system font files, then reinstall the chain.
    Load(Vec<&'static str>),
    /// Re-scan the user font folder and reinstall the chain.
    Reload,
}

/// A completed worker pass.
struct FontUpdate {
    report: FontReport,
    /// Every system file the worker has attempted (including failures), so the
    /// UI never asks for the same file twice.
    handled: Vec<&'static str>,
}

/// Owns the font worker and mirrors its latest report for the settings page.
pub struct FontManager {
    report: FontReport,
    handled: HashSet<&'static str>,
    requested: HashSet<&'static str>,
    request_tx: Sender<FontRequest>,
    update_rx: Receiver<FontUpdate>,
}

impl FontManager {
    /// Start the worker. It installs bundled + user + base system fonts first,
    /// then waits for on-demand script requests.
    pub fn spawn(ctx: egui::Context) -> Self {
        let (request_tx, request_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("font-load".to_string())
            .spawn(move || font_worker(ctx, request_rx, update_tx));
        Self {
            report: FontReport {
                custom_dir: custom_font_dir(),
                ..FontReport::default()
            },
            handled: HashSet::new(),
            requested: HashSet::new(),
            request_tx,
            update_rx,
        }
    }

    /// Drain worker results. Returns the newest report when one arrived.
    pub fn poll(&mut self) -> Option<FontReport> {
        let mut latest = None;
        while let Ok(update) = self.update_rx.try_recv() {
            self.report = update.report;
            self.handled = update.handled.into_iter().collect();
            self.requested
                .retain(|file| !self.handled.contains(file));
            latest = Some(self.report.clone());
        }
        latest
    }

    /// Load any script font that `text` needs. Cheap when everything is
    /// already installed; the actual disk read happens on the font worker.
    pub fn ensure_text(&mut self, text: &str) {
        let mask = scripts_in(text);
        if mask == 0 {
            return;
        }
        let mut needed = Vec::new();
        for script in Script::ALL {
            if mask & script.bit() == 0 {
                continue;
            }
            let file = script.file();
            if self.handled.contains(file) || self.requested.contains(file) {
                continue;
            }
            self.requested.insert(file);
            needed.push(file);
        }
        if !needed.is_empty() {
            let _ = self.request_tx.send(FontRequest::Load(needed));
        }
    }

    /// Re-scan the user font folder (Settings → Reload fonts).
    pub fn reload(&mut self) {
        let _ = self.request_tx.send(FontRequest::Reload);
    }
}

/// Install the bundled faces synchronously so the first frame has the real
/// ShuKanji look. User + system faces arrive from [`FontManager::spawn`].
pub fn install_bundled(ctx: &egui::Context) {
    let mut fonts = bundled();
    let defaults = fonts.families.clone();
    order_families(&mut fonts, &defaults, &[], &[]);
    ctx.set_fonts(fonts);
}

/// The user font folder.
pub fn custom_font_dir() -> Option<PathBuf> {
    paths::config_dir().map(|dir| dir.join("fonts"))
}

struct Worker {
    ctx: egui::Context,
    tx: Sender<FontUpdate>,
    fonts: FontDefinitions,
    /// The pristine egui default families, captured before any re-ordering.
    defaults: BTreeMap<FontFamily, Vec<String>>,
    custom_dir: Option<PathBuf>,
    custom_ids: Vec<String>,
    custom_names: Vec<String>,
    system_ids: Vec<String>,
    system_names: Vec<String>,
    handled: HashSet<&'static str>,
    custom_failed: Vec<String>,
    system_failed: Vec<String>,
}

fn font_worker(ctx: egui::Context, rx: Receiver<FontRequest>, tx: Sender<FontUpdate>) {
    let fonts = bundled();
    let defaults = fonts.families.clone();
    let mut worker = Worker {
        ctx,
        tx,
        fonts,
        defaults,
        custom_dir: custom_font_dir(),
        custom_ids: Vec::new(),
        custom_names: Vec::new(),
        system_ids: Vec::new(),
        system_names: Vec::new(),
        handled: HashSet::new(),
        custom_failed: Vec::new(),
        system_failed: Vec::new(),
    };

    worker.load_custom();
    for file in BASE_FONTS {
        worker.load_system(file);
    }
    worker.reinstall();
    worker.publish();

    while let Ok(request) = rx.recv() {
        match request {
            FontRequest::Load(files) => {
                let mut changed = false;
                for file in files {
                    if worker.handled.contains(file) {
                        continue;
                    }
                    if worker.load_system(file) {
                        changed = true;
                    }
                }
                if changed {
                    worker.reinstall();
                }
                worker.publish();
            }
            FontRequest::Reload => {
                worker.custom_failed.clear();
                worker.load_custom();
                worker.reinstall();
                worker.publish();
            }
        }
        worker.ctx.request_repaint();
    }
}

impl Worker {
    /// Rebuild both family lists from the pristine defaults plus everything
    /// currently installed. Safe to call after every change: it never
    /// compounds the previous order.
    fn reinstall(&mut self) {
        order_families(
            &mut self.fonts,
            &self.defaults,
            &self.custom_ids,
            &self.system_ids,
        );
        self.ctx.set_fonts(self.fonts.clone());
    }

    fn load_custom(&mut self) {
        self.fonts
            .font_data
            .retain(|id, _| !id.starts_with("custom/"));
        self.custom_ids.clear();
        self.custom_names.clear();

        let Some(dir) = self.custom_dir.clone() else {
            return;
        };
        for path in scan_font_files(&dir) {
            let name = file_label(&path);
            match std::fs::read(&path) {
                Ok(bytes) if !bytes.is_empty() => {
                    let id = format!("custom/{name}");
                    self.fonts
                        .font_data
                        .insert(id.clone(), Arc::new(FontData::from_owned(bytes)));
                    self.custom_ids.push(id);
                    self.custom_names.push(name);
                }
                _ => self.custom_failed.push(name),
            }
        }
    }

    fn load_system(&mut self, file: &'static str) -> bool {
        self.handled.insert(file);
        let Some(candidate) = SYSTEM_CANDIDATES.iter().find(|c| c.file == file) else {
            return false;
        };
        let path = Path::new(r"C:\Windows\Fonts").join(file);
        match std::fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => {
                let id = format!("system/{file}");
                self.fonts
                    .font_data
                    .insert(id.clone(), Arc::new(FontData::from_owned(bytes)));
                self.system_ids.push(id);
                self.system_names.push(candidate.name.to_owned());
                true
            }
            _ => {
                self.system_failed.push(candidate.name.to_owned());
                false
            }
        }
    }

    fn publish(&self) {
        let mut failed = self.custom_failed.clone();
        failed.extend(self.system_failed.iter().cloned());
        let report = FontReport {
            custom_dir: self.custom_dir.clone(),
            custom: self.custom_names.clone(),
            system: self.system_names.clone(),
            failed,
        };
        let _ = self.tx.send(FontUpdate {
            report,
            handled: self.handled.iter().copied().collect(),
        });
    }
}

/// Order matters: primary face, user fonts, system fallbacks, the egui
/// defaults that shipped with FontDefinitions, then monochrome emoji.
fn order_families(
    fonts: &mut FontDefinitions,
    defaults: &BTreeMap<FontFamily, Vec<String>>,
    custom_ids: &[String],
    system_ids: &[String],
) {
    for (family, primary) in [
        (FontFamily::Proportional, "inter"),
        (FontFamily::Monospace, "jetbrains"),
    ] {
        let base = defaults.get(&family).cloned().unwrap_or_default();
        let mut ordered = Vec::new();
        ordered.push(primary.to_owned());
        ordered.extend(custom_ids.iter().cloned());
        ordered.extend(system_ids.iter().cloned());
        ordered.extend(base);
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

/// Windows faces used by the fallback chain. Base faces load at start-up; the
/// rest are loaded by [`Script`] on demand. `.ttc` collections load their first
/// face. Redundant Latin/Chinese faces (Arial, Tahoma, SimSun, JhengHei) were
/// dropped: Segoe UI + Microsoft YaHei cover them and cost less RAM.
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
        file: "msyh.ttc",
        name: "Microsoft YaHei (Chinese)",
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
        file: "seguiemj.ttf",
        name: "Segoe UI Emoji",
    },
];

/// Which on-demand scripts appear in `text`.
///
/// Kana wins over Han so a Japanese sentence only pulls the Japanese face
/// (Han without kana is treated as Chinese). Hangul, Thai, Indic and emoji are
/// independent.
fn scripts_in(text: &str) -> u8 {
    let mut mask = 0;
    let mut kana = false;
    let mut han = false;

    for ch in text.chars() {
        let c = ch as u32;
        if (0x3040..=0x30FF).contains(&c)
            || (0x31F0..=0x31FF).contains(&c)
            || (0xFF66..=0xFF9D).contains(&c)
        {
            kana = true;
        } else if (0x3400..=0x4DBF).contains(&c)
            || (0x4E00..=0x9FFF).contains(&c)
            || (0xF900..=0xFAFF).contains(&c)
        {
            han = true;
        } else if (0xAC00..=0xD7AF).contains(&c)
            || (0x1100..=0x11FF).contains(&c)
            || (0x3130..=0x318F).contains(&c)
        {
            mask |= Script::Korean.bit();
        } else if (0x0E00..=0x0E7F).contains(&c) {
            mask |= Script::Thai.bit();
        } else if (0x0900..=0x0DFF).contains(&c) {
            mask |= Script::Indic.bit();
        } else if (0x1F000..=0x1FAFF).contains(&c)
            || (0x2600..=0x27BF).contains(&c)
            || (0x1F1E6..=0x1F1FF).contains(&c)
        {
            mask |= Script::Emoji.bit();
        }
    }

    if kana {
        mask |= Script::Japanese.bit();
    } else if han {
        mask |= Script::Chinese.bit();
    }
    mask
}

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
    fn detects_only_the_scripts_present() {
        assert_eq!(scripts_in("hello world"), 0);
        assert_eq!(scripts_in("こんにちは"), Script::Japanese.bit());
        assert_eq!(scripts_in("你好世界"), Script::Chinese.bit());
        assert_eq!(scripts_in("안녕하세요"), Script::Korean.bit());
        assert_eq!(scripts_in("สวัสดี"), Script::Thai.bit());
        assert_eq!(scripts_in("नमस्ते"), Script::Indic.bit());
        assert_eq!(scripts_in("😀🎉"), Script::Emoji.bit());
        // Japanese kanji + kana must not also pull the Chinese face.
        let japanese = scripts_in("日本語のチャット");
        assert_ne!(japanese & Script::Japanese.bit(), 0);
        assert_eq!(japanese & Script::Chinese.bit(), 0);
    }

    #[test]
    fn every_script_has_an_installed_candidate() {
        for script in Script::ALL {
            let file = script.file();
            assert!(
                SYSTEM_CANDIDATES.iter().any(|c| c.file == file),
                "missing candidate for {file}"
            );
        }
    }

    #[test]
    fn family_order_puts_primary_custom_system_and_emoji_in_place() {
        let mut fonts = FontDefinitions::default();
        let defaults = fonts.families.clone();
        let custom = vec!["custom/mine.ttf".to_owned()];
        let system = vec!["system/segoeui.ttf".to_owned()];
        order_families(&mut fonts, &defaults, &custom, &system);

        let proportional = fonts
            .families
            .get(&FontFamily::Proportional)
            .expect("proportional family");
        assert_eq!(proportional.first().map(String::as_str), Some("inter"));
        assert_eq!(proportional.last().map(String::as_str), Some("notoemoji"));
        let custom_pos = proportional
            .iter()
            .position(|id| id == "custom/mine.ttf")
            .unwrap();
        let system_pos = proportional
            .iter()
            .position(|id| id == "system/segoeui.ttf")
            .unwrap();
        assert!(custom_pos < system_pos);

        let mono = fonts
            .families
            .get(&FontFamily::Monospace)
            .expect("monospace family");
        assert_eq!(mono.first().map(String::as_str), Some("jetbrains"));
        assert_eq!(mono.last().map(String::as_str), Some("notoemoji"));
    }

    #[test]
    fn reordering_does_not_compound() {
        let mut fonts = FontDefinitions::default();
        let defaults = fonts.families.clone();
        let custom = vec!["custom/mine.ttf".to_owned()];
        let system = vec!["system/segoeui.ttf".to_owned()];
        order_families(&mut fonts, &defaults, &custom, &system);
        let first = fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap();
        order_families(&mut fonts, &defaults, &custom, &system);
        let second = fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap();
        assert_eq!(first, second);
    }
}
