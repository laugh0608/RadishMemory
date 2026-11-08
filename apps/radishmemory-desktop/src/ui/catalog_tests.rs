use super::{test_support::Harness, *};
use crate::worker::tests::{Fixture, ok};
use crate::{
    ApplicationPaths, ProductionRuntime, ProductionRuntimeError, load_or_create_host_profile,
};
use radishmemory_application::{
    ApplicationIdentifierKind, ApplicationRuntime, DeletionOverallStatus, FileExportRequest,
    LocalLibrary, LocalLibraryConfig, Timestamp,
};
use std::fs;

const SOURCE_COUNT: usize = 201;

struct CatalogRuntime;

impl ApplicationRuntime for CatalogRuntime {
    type Error = ProductionRuntimeError;

    fn next_identifier(
        &mut self,
        kind: ApplicationIdentifierKind,
    ) -> Result<Identifier, Self::Error> {
        ProductionRuntime.next_identifier(kind)
    }

    fn now(&mut self) -> Result<Timestamp, Self::Error> {
        Ok(Timestamp::parse("2000-01-01T12:00:00Z").unwrap())
    }
}

fn seed(fixture: &Fixture) -> Vec<(Identifier, Identifier, String)> {
    let paths = ApplicationPaths::from_data_directory(fixture.root.join("data")).unwrap();
    let mut runtime = CatalogRuntime;
    let profile = load_or_create_host_profile(&paths, &mut runtime).unwrap();
    let config = LocalLibraryConfig::phase1_local(
        profile.namespace_id().clone(),
        profile.device_id().clone(),
    )
    .unwrap();
    let mut library = LocalLibrary::open(paths.database_path(), runtime, config).unwrap();
    let mut sources = Vec::new();
    for index in 0..SOURCE_COUNT {
        let token = format!("catalogentry{index:03}");
        let request = fixture.named_input(&format!("{token}.md"), &token);
        let receipt = library.import_new_source(&request).unwrap();
        sources.push((
            receipt.lineage_id().clone(),
            receipt.source_id().clone(),
            token,
        ));
    }
    // All captures share a timestamp. The oracle uses the write receipts, not the catalog.
    sources.sort_by(|(left, _, _), (right, _, _)| left.as_str().cmp(right.as_str()));
    sources
}

fn exercise_catalog(encrypted: bool) {
    let fixture = Fixture::new();
    let expected = seed(&fixture);
    let (lineage, source, query) = expected.last().unwrap();
    let mut engine = fixture.engine();
    let opened = if encrypted {
        ok(engine.execute(Command::InitializeKey));
        ok(engine.execute(Command::Migrate));
        ok(engine.execute(Command::OpenEncrypted))
    } else {
        ok(engine.execute(Command::OpenPlain))
    };
    let first_page = opened.view.as_ref().unwrap();
    let expected_first_page = expected[..SOURCE_COUNT - 1]
        .iter()
        .map(|(lineage, _, _)| lineage)
        .collect::<Vec<_>>();
    assert_eq!(
        first_page
            .sources()
            .iter()
            .map(|row| row.lineage_id())
            .collect::<Vec<_>>(),
        expected_first_page
    );

    let mut harness = Harness::new();
    harness.results.send(opened).unwrap();
    harness
        .click("Previous page")
        .drop_without_applying_deltas();
    harness.no_command();
    harness.click("Next page").drop_without_applying_deltas();
    let last_page = ok(engine.execute(harness.commands.try_recv().unwrap()));
    let last_view = last_page.view.as_ref().unwrap();
    assert_eq!(last_view.catalog_offset(), 200);
    assert_eq!(last_view.sources().len(), 1);
    assert_eq!(last_view.sources()[0].lineage_id(), lineage);
    assert_eq!(last_view.selected_source_id(), Some(source));
    harness.results.send(last_page).unwrap();
    harness.click("Next page").drop_without_applying_deltas();
    harness.no_command();
    harness
        .click("Previous page")
        .drop_without_applying_deltas();
    let first_page = ok(engine.execute(harness.commands.try_recv().unwrap()));
    assert_eq!(
        first_page
            .view
            .as_ref()
            .unwrap()
            .sources()
            .iter()
            .map(|row| row.lineage_id())
            .collect::<Vec<_>>(),
        expected_first_page
    );
    harness.results.send(first_page).unwrap();

    harness.app.search_query = query.clone();
    harness.click("Search").drop_without_applying_deltas();
    let searched = ok(engine.execute(harness.commands.try_recv().unwrap()));
    let results = searched.view.as_ref().unwrap().search_results();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source_id(), source);
    let link = format!(
        "{} · v1 · bytes 0..{}",
        results[0].title().unwrap().as_str(),
        results[0].byte_end()
    );
    harness.results.send(searched).unwrap();
    harness.click(&link).drop_without_applying_deltas();
    let selected = ok(engine.execute(harness.commands.try_recv().unwrap()));
    let view = selected.view.as_ref().unwrap();
    assert_eq!(view.catalog_offset(), 0);
    assert!(view.selected_lineage().is_none());
    assert_eq!(view.selected_lineage_id(), Some(lineage));
    assert_eq!(view.selected_source_id(), Some(source));
    harness.results.send(selected).unwrap();
    let frame = harness.frame(false, vec![]);
    for label in [
        "Selected source is outside this catalog page.",
        "Update from file…",
        "Export selected version…",
        "Delete managed lineage…",
    ] {
        super::test_support::label_position(&frame, label);
    }
    frame.drop_without_applying_deltas();

    // Supply picker results at the production worker boundary, with explicit roots.
    let revised_bytes = "\u{feff}catalogrevised\r\n";
    let update = fixture.named_input("revised.md", revised_bytes);
    let updated = ok(engine.execute(Command::Update(update)));
    let view = updated.view.as_ref().unwrap();
    assert_eq!(view.selected_lineage_id(), Some(lineage));
    assert_eq!(view.versions().len(), 2);
    let revised_source = view.selected_source_id().unwrap().clone();
    assert_ne!(&revised_source, source);
    assert!(view.search_results().is_empty());

    // The update moves the lineage to page one. Select it from page two before
    // exporting or deleting, so neither action can rely on a cached catalog row.
    harness.results.send(updated).unwrap();
    harness.click("Next page").drop_without_applying_deltas();
    let last_page = ok(engine.execute(harness.commands.try_recv().unwrap()));
    assert_eq!(last_page.view.unwrap().catalog_offset(), 200);
    let searched = ok(engine.execute(Command::Search("catalogrevised".into())));
    let results = searched.view.as_ref().unwrap().search_results();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source_id(), &revised_source);
    let selected = ok(engine.execute(Command::SelectResult(
        lineage.clone(),
        revised_source.clone(),
        results[0].fragment_id().clone(),
    )));
    let view = selected.view.as_ref().unwrap();
    assert_eq!(view.catalog_offset(), 200);
    assert!(view.selected_lineage().is_none());
    assert_eq!(view.selected_source_id(), Some(&revised_source));
    let export_path = fixture.root.join("export/revised.md");
    let export = FileExportRequest::new(&export_path, vec![fixture.root.join("export")]).unwrap();
    ok(engine.execute(Command::Export(export)));
    assert_eq!(fs::read(&export_path).unwrap(), revised_bytes.as_bytes());

    harness.results.send(selected).unwrap();
    harness
        .click("Delete managed lineage…")
        .drop_without_applying_deltas();
    assert!(harness.app.confirm_delete);
    harness
        .click("Delete managed lineage")
        .drop_without_applying_deltas();
    let command = harness.commands.try_recv().unwrap();
    assert!(matches!(command, Command::Delete));
    let deleted = ok(engine.execute(command));
    assert_eq!(
        deleted.last_deletion.unwrap().params().overall_status,
        DeletionOverallStatus::Completed
    );
    let view = deleted.view.unwrap();
    assert!(!view.has_next_catalog_page());
    assert_eq!(view.sources().len(), 200);
    assert!(view.sources().iter().all(|row| row.lineage_id() != lineage));
    assert_eq!(fs::read(&export_path).unwrap(), revised_bytes.as_bytes());
    drop(engine);

    let mut reopened = fixture.engine();
    ok(reopened.execute(if encrypted {
        Command::OpenEncrypted
    } else {
        Command::OpenPlain
    }));
    let searched = ok(reopened.execute(Command::Search("catalogrevised".into())));
    assert!(searched.view.unwrap().search_results().is_empty());
    ok(reopened.execute(Command::Verify));
}

#[test]
fn plain_catalog_paginates_and_manages_a_source_found_off_page() {
    exercise_catalog(false);
}

#[test]
fn encrypted_catalog_paginates_and_manages_a_source_found_off_page() {
    exercise_catalog(true);
}
