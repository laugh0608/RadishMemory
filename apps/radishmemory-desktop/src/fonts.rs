//! Explicit bundled glyph coverage; no system-font lookup or runtime downloads.
use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};

const CHINESE_FONT: &str = "Noto Sans SC Regular 2.004";

pub(crate) fn install(context: &Context) {
    let mut definitions = FontDefinitions::default();
    definitions.font_data.insert(
        CHINESE_FONT.into(),
        FontData::from_static(include_bytes!(
            "../../../third_party/fonts/noto-sans-sc/NotoSansSC-Regular.otf"
        ))
        .into(),
    );
    // Retain the existing Latin / emoji faces and add the reviewed Chinese face.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        definitions
            .families
            .entry(family)
            .or_default()
            .push(CHINESE_FONT.into());
    }
    context.set_fonts(definitions);
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Color32, FontId, RawInput};

    const CHINESE: &str =
        "资料库：项目记忆、更正与忘记。来源引用；历史版本（中文检索）！繁體資料，記憶與刪除。";
    const MIXED: &str = "项目 RadishMemory 2026：API / UTF-8；来源 v2 已更正。\n繁體記憶與歷史版本，导出 Markdown。";

    #[test]
    fn bundled_chinese_renders_without_changing_existing_latin_metrics() {
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let id = FontId::new(16.0, family.clone());
            let baseline = Context::default();
            let mut latin_size = None;
            baseline
                .run_ui(RawInput::default(), |ui| {
                    ui.ctx().fonts_mut(|fonts| {
                        assert!(!fonts.fonts.font(&family).characters().contains_key(&'资'));
                        latin_size = Some(
                            fonts
                                .layout_no_wrap(
                                    "RadishMemory API 2026".into(),
                                    id.clone(),
                                    Color32::WHITE,
                                )
                                .size(),
                        );
                    });
                })
                .drop_without_applying_deltas();
            let context = Context::default();
            install(&context);
            context
                .run_ui(RawInput::default(), |ui| {
                    ui.ctx().fonts_mut(|fonts| {
                        // epaint 0.36.1 has_glyph compares the owning face to the replacement
                        // face, yielding false negatives for supported glyphs on that face.
                        // Use the actual character mapping, then exercise shaping/rasterization.
                        let characters = fonts.fonts.font(&family).characters().clone();
                        for ch in CHINESE.chars() {
                            assert!(
                                characters.get(&ch).is_some_and(|faces| faces
                                    .iter()
                                    .any(|name| name == CHINESE_FONT)),
                                "missing bundled glyph: {ch}"
                            );
                        }
                        for ch in MIXED.chars().filter(|ch| !ch.is_whitespace()) {
                            assert!(characters.contains_key(&ch), "missing mixed glyph: {ch}");
                        }
                        let latin = fonts.layout_no_wrap(
                            "RadishMemory API 2026".into(),
                            id.clone(),
                            Color32::WHITE,
                        );
                        assert_eq!(Some(latin.size()), latin_size);
                        let chinese =
                            fonts.layout_no_wrap(CHINESE.into(), id.clone(), Color32::WHITE);
                        assert!(!chinese.elided);
                        assert!(chinese.num_vertices > 0);
                        let missing =
                            fonts.layout_no_wrap("\u{10ffff}".into(), id.clone(), Color32::WHITE);
                        assert_ne!(
                            chinese.rows[0].glyphs[0].uv_rect,
                            missing.rows[0].glyphs[0].uv_rect
                        );
                    });
                })
                .drop_without_applying_deltas();
        }
    }

    #[test]
    fn mixed_chinese_wraps_without_overlapping_rows_at_desktop_scales() {
        for scale in [1.0, 1.5, 2.0] {
            let context = Context::default();
            context.set_pixels_per_point(scale);
            install(&context);
            context
                .run_ui(RawInput::default(), |ui| {
                    ui.ctx().fonts_mut(|fonts| {
                        for family in [FontFamily::Proportional, FontFamily::Monospace] {
                            let galley = fonts.layout(
                                MIXED.into(),
                                FontId::new(16.0, family),
                                Color32::WHITE,
                                180.0,
                            );
                            assert!(!galley.elided);
                            assert!(galley.rows.len() > 2);
                            assert!(galley.size().x <= 180.0);
                            for rows in galley.rows.windows(2) {
                                let first =
                                    rows[0].visuals.mesh_bounds.translate(rows[0].pos.to_vec2());
                                let next =
                                    rows[1].visuals.mesh_bounds.translate(rows[1].pos.to_vec2());
                                assert!(
                                    first.bottom() <= next.top(),
                                    "overlapping glyph rows at scale {scale}"
                                );
                            }
                        }
                    });
                })
                .drop_without_applying_deltas();
        }
    }
}
