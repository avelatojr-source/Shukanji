# Fonts

| File | Family | Used for | Notes |
|---|---|---|---|
| `Inter.ttf` | Inter | all UI text | full face — chat can contain any script, so this must stay complete |
| `JetBrainsMono.ttf` | JetBrains Mono | tokens, keys, log lines | full face |
| `NotoEmoji.ttf` | Noto Emoji | monochrome emoji fallback | full face — any emoji can appear, so it cannot be subset |
| `ZenAntique-Wordmark.ttf` | Zen Antique | the "ShuKanji" wordmark only | **subset**, see below |

## Why `ZenAntique-Wordmark.ttf` is a subset

The full Zen Antique Regular is 5.4 MB, almost entirely CJK outlines. The app
uses that family for exactly one string — the wordmark at the top of the sidebar
(`panels.rs`, `RichText::new("ShuKanji").family(theme::wordmark_family())`) — so
the rest was 5.5 MB of dead weight in every build.

The shipped file keeps printable ASCII (`U+0020`–`U+007E`), which is 17 KB.

Regenerate it from the full face with:

```bash
pip install fonttools
pyftsubset ZenAntique-Regular.ttf \
    --unicodes=U+0020-007E \
    --output-file=ZenAntique-Wordmark.ttf \
    --layout-features= --no-hinting --desubroutinize
```

The subset was verified to keep all 95 printable ASCII glyphs, to cover every
character of `"ShuKanji"`, and to keep real outlines (not just cmap entries) for
each of those characters. Verified with:

```python
from fontTools.ttLib import TTFont
font = TTFont("ZenAntique-Wordmark.ttf")
cmap = font.getBestCmap()
glyf = font.get("glyf")
for c in "ShuKanji":
    assert ord(c) in cmap, c
    assert glyf[cmap[ord(c)]].numberOfContours > 0, c
```

Source of the full face: Google Fonts, `ofl/zenantique/ZenAntique-Regular.ttf`.

**If you ever render non-ASCII text in the wordmark family, regenerate the
subset with a wider range** — otherwise those characters will have no glyph.
