//! Colour emoji rendering.
//!
//! egui's text engine can only draw monochrome glyphs, so real colour emoji are
//! rendered as PNG images fetched through the shared [`ImageCache`]. While an
//! emoji image is still downloading we draw the monochrome glyph instead, so a
//! comment never shows a blank gap.
//!
//! The image assets come from `emoji-datasource-twitter`, whose file names are
//! the fully-qualified emoji sequence in lowercase hex joined by `-` — fully
//! deterministic, with no guessing about variation selectors.

use egui::{Color32, RichText, Vec2};

use super::image_cache::ImageCache;

/// CDN prefix for the Twitter-style colour emoji PNGs (Emoji 15.1 assets).
pub const EMOJI_BASE: &str =
    "https://cdn.jsdelivr.net/npm/emoji-datasource-twitter@15.1.2/img/twitter/64/";

/// One drawable piece of a chat message.
pub enum Piece {
    Text(String),
    /// A remote image plus the text shown until it has loaded.
    Image {
        url: String,
        /// Empty for custom TikTok emotes (nothing is drawn while loading).
        fallback: String,
        size: f32,
    },
}

/// Emoji that are always emoji-presentation (no U+FE0F required).
fn is_emoji_core(c: u32) -> bool {
    matches!(
        c,
        0x1F000..=0x1FAFF      // pictographs, emoticons, transport, symbols, extended
        | 0x2600..=0x27BF      // misc symbols + dingbats
        | 0x2B00..=0x2BFF      // arrows/stars/shapes
    )
}

/// Characters that only become emoji when followed by U+FE0F.
fn is_emoji_with_vs(c: u32) -> bool {
    matches!(
        c,
        0x00A9
            | 0x00AE
            | 0x203C
            | 0x2049
            | 0x2122
            | 0x2139
            | 0x2194..=0x2199
            | 0x21A9..=0x21AA
            | 0x231A..=0x231B
            | 0x2328
            | 0x23CF
            | 0x23E9..=0x23FA
            | 0x24C2
            | 0x25AA..=0x25AB
            | 0x25B6
            | 0x25C0
            | 0x25FB..=0x25FE
            | 0x2934..=0x2935
            | 0x3030
            | 0x303D
            | 0x3297
            | 0x3299
    )
}

fn is_regional(c: u32) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&c)
}

/// The asset URL for a fully-qualified emoji sequence.
pub fn emoji_url(codepoints: &[u32]) -> String {
    let mut url = String::with_capacity(EMOJI_BASE.len() + codepoints.len() * 6);
    url.push_str(EMOJI_BASE);
    for (i, cp) in codepoints.iter().enumerate() {
        if i > 0 {
            url.push('-');
        }
        let _ = std::fmt::Write::write_fmt(&mut url, format_args!("{cp:x}"));
    }
    url.push_str(".png");
    url
}

/// How many `char`s from `start` form one emoji cluster.
fn cluster_len(chars: &[char], start: usize) -> usize {
    let base = chars[start] as u32;
    let mut i = start + 1;

    // A flag is exactly two regional indicators.
    if is_regional(base) {
        if let Some(&next) = chars.get(i)
            && is_regional(next as u32)
        {
            i += 1;
        }
        return i - start;
    }

    while let Some(&c) = chars.get(i) {
        let cc = c as u32;
        match cc {
            // Variation selectors.
            0xFE0E | 0xFE0F => i += 1,
            // Skin-tone modifiers.
            0x1F3FB..=0x1F3FF => i += 1,
            // Keycap.
            0x20E3 => i += 1,
            // Subdivision-flag tag characters.
            0xE0020..=0xE007F => i += 1,
            0x200D => {
                // Zero-width joiner: keep it only if another emoji follows.
                let Some(&next) = chars.get(i + 1) else {
                    break;
                };
                let nc = next as u32;
                let more = is_emoji_core(nc)
                    || is_emoji_with_vs(nc)
                    || matches!(nc, 0xFE0F | 0x1F3FB..=0x1F3FF);
                if more {
                    i += 2;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    i - start
}

/// Split a run of text into text and emoji pieces.
pub fn split(text: &str, size: f32) -> Vec<Piece> {
    let chars: Vec<char> = text.chars().collect();
    let mut pieces: Vec<Piece> = Vec::new();
    let mut buffer = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let cc = c as u32;
        let starts_emoji = is_emoji_core(cc)
            || (is_emoji_with_vs(cc) && chars.get(i + 1).is_some_and(|&n| n as u32 == 0xFE0F));
        if starts_emoji {
            let len = cluster_len(&chars, i);
            if !buffer.is_empty() {
                pieces.push(Piece::Text(std::mem::take(&mut buffer)));
            }
            let codepoints: Vec<u32> = chars[i..i + len].iter().map(|&c| c as u32).collect();
            let fallback: String = chars[i..i + len].iter().collect();
            pieces.push(Piece::Image {
                url: emoji_url(&codepoints),
                fallback,
                size,
            });
            i += len;
        } else {
            buffer.push(c);
            i += 1;
        }
    }
    if !buffer.is_empty() {
        pieces.push(Piece::Text(buffer));
    }
    pieces
}

/// Draw the pieces inline (call inside a `horizontal_wrapped` layout).
pub fn render(ui: &mut egui::Ui, images: &mut ImageCache, pieces: &[Piece], color: Color32) {
    for piece in pieces {
        match piece {
            Piece::Text(text) => {
                ui.label(RichText::new(text).color(color).size(16.0));
            }
            Piece::Image {
                url,
                fallback,
                size,
            } => match images.uri(url) {
                Some(uri) => {
                    ui.add(
                        egui::Image::from_uri(uri)
                            .fit_to_exact_size(Vec2::splat(*size))
                            .show_loading_spinner(false),
                    );
                }
                None if !fallback.is_empty() => {
                    ui.label(RichText::new(fallback).color(color).size(16.0));
                }
                None => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(pieces: &[Piece]) -> Vec<String> {
        pieces
            .iter()
            .map(|p| match p {
                Piece::Text(t) => format!("T:{t}"),
                Piece::Image { url, .. } => format!("I:{url}"),
            })
            .collect()
    }

    #[test]
    fn plain_text_is_one_piece() {
        let pieces = split("hello world", 16.0);
        assert_eq!(texts(&pieces), vec!["T:hello world"]);
    }

    #[test]
    fn splits_a_simple_emoji() {
        let pieces = split("a 🤤 b", 16.0);
        assert_eq!(
            texts(&pieces),
            vec![
                "T:a ".to_string(),
                format!("I:{EMOJI_BASE}1f924.png"),
                "T: b".to_string(),
            ]
        );
    }

    #[test]
    fn keeps_variation_selector_in_the_asset_name() {
        let pieces = split("❤️", 16.0);
        assert_eq!(texts(&pieces), vec![format!("I:{EMOJI_BASE}2764-fe0f.png")]);
    }

    #[test]
    fn pairs_regional_indicators_into_one_flag() {
        let pieces = split("🇺🇸", 16.0);
        assert_eq!(
            texts(&pieces),
            vec![format!("I:{EMOJI_BASE}1f1fa-1f1f8.png")]
        );
    }

    #[test]
    fn keeps_skin_tone_and_zwj_together() {
        let pieces = split("👩🏻‍💻", 16.0);
        assert_eq!(
            texts(&pieces),
            vec![format!("I:{EMOJI_BASE}1f469-1f3fb-200d-1f4bb.png")]
        );
    }

    #[test]
    fn digits_are_not_emoji() {
        let pieces = split("1234", 16.0);
        assert_eq!(texts(&pieces), vec!["T:1234"]);
    }
}
