//! Chinese text from the encrypted application path reaches the real UI widgets.
use super::test_support::Harness;
use super::*;
use crate::worker::tests::{Fixture, ok, ready};

#[test]
fn chinese_source_title_query_and_result_preview_are_painted_with_bundled_glyphs() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    ok(engine.execute(Command::Import(fixture.named_input(
        "项目资料-v2.md",
        "中文资料与繁體記憶，API 2026；displaytoken 精确导出。",
    ))));
    // ASCII retrieval isolates display coverage; it does not close Q01 retrieval quality.
    let result = ok(engine.execute(Command::Search("displaytoken".into())));
    assert_eq!(result.view.as_ref().unwrap().search_results().len(), 1);
    let mut h = Harness::new();
    h.app.search_query = "中文 displaytoken".into();
    h.results.send(result).unwrap();
    let output = h.frame(false, vec![]);
    let text = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>();
    for expected in ["项目资料-v2.md", "中文 displaytoken", "中文资料与繁體記憶"] {
        assert!(
            text.iter()
                .any(|text| text.galley.text().contains(expected))
        );
    }
    h.context.fonts_mut(|fonts| {
        for shape in text {
            for section in &shape.galley.job.sections {
                let mapped = fonts
                    .fonts
                    .font(&section.format.font_id.family)
                    .characters()
                    .clone();
                let range = section.byte_range.start.0..section.byte_range.end.0;
                for ch in shape.galley.text()[range].chars() {
                    if ('\u{4e00}'..='\u{9fff}').contains(&ch) {
                        assert!(mapped.contains_key(&ch), "painted text lacks glyph: {ch}");
                    }
                }
            }
        }
    });
    output.drop_without_applying_deltas();
    h.no_command();
}
