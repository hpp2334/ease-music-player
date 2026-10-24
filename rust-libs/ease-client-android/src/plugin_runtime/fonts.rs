//! CJK fallback fonts for the tur text pipeline.
//!
//! tur's default [`tur_native::NativeFontLoader`] only calls fontique's
//! `load_system_fonts()`, which on Android discovers fonts via
//! `/system/etc/fonts.xml` + `/system/fonts`. That path is fragile: the
//! script-fallback registration requires every `<family lang="zh-Hans">`-style
//! entry in fonts.xml to carry a `postScriptName` that string-matches a scanned
//! font, and ROMs get this wrong (observed on an Android 16 Mi 11: the
//! simplified-only Han characters — 务称别录户码变测试… — rendered as tofu in
//! every plugin view while shared-script Han still resolved, i.e. the Hans
//! fallback silently never registered).
//!
//! This loader keeps the system font scan (Latin etc. come from Roboto) and
//! additionally registers a device CJK font file directly — bypassing
//! fonts.xml entirely — then points the CJK scripts (`Hans`/`Hant`/`Hani`/
//! `Jpan`/`Kore`) at it explicitly and appends it to the `sans-serif` generic
//! family as a last-resort primary candidate. Any ROM that can display CJK at
//! all ships one of the known font files, so plugin text never depends on the
//! fonts.xml quirk again.

use std::path::PathBuf;

use fontique::{FallbackKey, GenericFamily, Script};
use tur_engine::core::fonts::{FontContext, FontLoader};

/// Font loader installed on the shared tur runtime
/// ([`crate::plugin_runtime::plugin_jni`] — `createRuntime`'s configure
/// closure chains `.font_loader(..)` over tur's default). Unit struct: all
/// state lives in the [`FontContext`] handed to [`FontLoader::load_preset_fonts`].
#[derive(Debug, Default, Clone, Copy)]
pub struct EaseFontLoader;

impl FontLoader for EaseFontLoader {
    /// System fonts first (fontique's lazy scan — Roboto & friends for
    /// Latin), then a directly-registered CJK font as a fonts.xml-independent
    /// fallback. Runs once, on the tur-host thread, while the shared runtime
    /// builds its [`FontContext`].
    fn load_preset_fonts(&self, fcx: &mut FontContext) {
        fcx.collection.load_system_fonts();
        match register_cjk_fallback(fcx) {
            Ok(Some(families)) => {
                let names: Vec<String> = families
                    .iter()
                    .map(|id| {
                        fcx.collection
                            .family_name(*id)
                            .unwrap_or("?")
                            .to_string()
                    })
                    .collect();
                tracing::info!("cjk fallback font registered: {names:?}");
            }
            Ok(None) => tracing::warn!(
                "no CJK font found under the system fonts dir — plugin views may \
                 render CJK as tofu on this ROM"
            ),
            Err(e) => tracing::warn!("cjk fallback font registration failed: {e}"),
        }
    }
}

/// Ordered, lowercased substrings identifying CJK-capable font files, best
/// first. Matched against font file names in the system fonts dir.
const CJK_FONT_HINTS: &[&str] = &[
    "notosanscjk",      // AOSP pan-CJK TTC (JP/KR/SC/TC/HK members)
    "notosanssc",       // per-script SC builds (some ROMs / newer AOSP)
    "misans",           // Xiaomi's system font on MIUI/HyperOS builds
    "droidsansfallback",// legacy CJK fallback
    "sourcehansans",    // Adobe Source Han Sans (= Noto Sans CJK)
    "notoserifcjk",     // serif pan-CJK — last resort, still full coverage
    "notosanstc",
];

/// Preference score for a font file name (lower = tried first). `None` = not
/// a CJK candidate. Pure so the ordering is unit-testable.
fn cjk_candidate_score(file_name: &str) -> Option<u8> {
    let name = file_name.to_ascii_lowercase();
    if name == "notosanscjk-regular.ttc" {
        return Some(0);
    }
    CJK_FONT_HINTS
        .iter()
        .position(|hint| name.contains(hint))
        .map(|position| (position as u8) + 1)
}

/// List CJK font files under the system fonts dir, best candidate first.
fn cjk_font_candidates() -> std::io::Result<Vec<PathBuf>> {
    let android_root = std::env::var("ANDROID_ROOT").unwrap_or_else(|_| "/system".to_string());
    let fonts_dir = PathBuf::from(android_root).join("fonts");
    let mut scored: Vec<(u8, PathBuf)> = std::fs::read_dir(&fonts_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().to_string();
            cjk_candidate_score(&name).map(|score| (score, path))
        })
        .collect();
    scored.sort();
    Ok(scored.into_iter().map(|(_, path)| path).collect())
}

/// Register the first readable CJK font into `fcx` and wire it up as the CJK
/// script fallback + last-resort `sans-serif` member. Returns the registered
/// family ids (CJK-script order, SC-preferred), or `None` when no candidate
/// file exists (e.g. host tests / a ROM without CJK fonts).
fn register_cjk_fallback(fcx: &mut FontContext) -> anyhow::Result<Option<Vec<fontique::FamilyId>>> {
    let Some(path) = cjk_font_candidates()?.into_iter().next() else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path)?;
    tracing::info!(
        "registering CJK fallback font: {} ({} bytes)",
        path.display(),
        bytes.len()
    );
    let registered = fcx.collection.register_fonts(bytes.into(), None);

    // Pan-CJK TTCs register one family per language member (JP/KR/SC/TC/HK);
    // they cover the same ideograph repertoire, so keep them all as fallbacks
    // but order SC first to match the glyphs a zh-Hans reader expects.
    let mut families: Vec<fontique::FamilyId> = registered.into_iter().map(|(id, _)| id).collect();
    families.sort_by_key(|id| {
        let name = fcx.collection.family_name(*id).unwrap_or("");
        (if name.contains("SC") { 0u8 } else { 1 }, name.to_string())
    });

    // Explicit script fallbacks — the whole point: these no longer depend on
    // fonts.xml postScriptName matching.
    for tag in [b"Hans", b"Hant", b"Hani", b"Jpan", b"Kore"] {
        fcx.collection.set_fallbacks(
            FallbackKey::new(Script::from_bytes(*tag), None),
            families.iter().copied(),
        );
    }
    // Belt and suspenders: also the last-resort primary candidate, so a Han
    // cluster resolves even if some other run-level fallback path regresses.
    // Appended (not set) — Latin still resolves from Roboto first.
    fcx.collection
        .append_generic_families(GenericFamily::SansSerif, families.iter().copied());
    fcx.collection
        .append_generic_families(GenericFamily::SystemUi, families.iter().copied());

    Ok(Some(families))
}

#[cfg(test)]
mod tests {
    use super::cjk_candidate_score;
    use tur_engine::core::fonts::FontLoader as _;

    #[test]
    fn prefers_the_regular_pan_cjk_ttc() {
        assert_eq!(cjk_candidate_score("NotoSansCJK-Regular.ttc"), Some(0));
        // Any other pan-CJK sans (e.g. a Bold-weight TTC) still beats the
        // serif last resort.
        assert!(
            cjk_candidate_score("NotoSansCJK-Bold.ttc").unwrap()
                < cjk_candidate_score("NotoSerifCJK-Regular.ttc").unwrap()
        );
    }

    #[test]
    fn orders_known_cjk_files_before_the_serif_fallback() {
        let mut names = [
            "NotoSerifCJK-Regular.ttc",
            "MiSans-Regular.ttf",
            "DroidSansFallback.ttf",
            "NotoSansSC-VF.ttf",
            "Roboto-Regular.ttf",
        ];
        names.sort_by_key(|n| cjk_candidate_score(n).unwrap_or(u8::MAX));
        assert_eq!(
            names,
            [
                "NotoSansSC-VF.ttf",
                "MiSans-Regular.ttf",
                "DroidSansFallback.ttf",
                "NotoSerifCJK-Regular.ttc",
                "Roboto-Regular.ttf",
            ]
        );
    }

    #[test]
    fn rejects_non_cjk_fonts() {
        assert_eq!(cjk_candidate_score("Roboto-Regular.ttf"), None);
        assert_eq!(cjk_candidate_score("NotoSansHebrew-Regular.ttf"), None);
    }

    #[test]
    fn load_preset_fonts_is_panic_free_without_a_cjk_font() {
        // Host: /system/fonts doesn't exist → the loader must degrade to a
        // warning, never panic (same path a CJK-less ROM would hit).
        let mut fcx = tur_engine::core::fonts::FontContext::default();
        super::EaseFontLoader.load_preset_fonts(&mut fcx);
    }
}
