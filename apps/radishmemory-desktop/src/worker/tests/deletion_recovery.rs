use super::*;
use SourceVaultErrorCode::{KeyStoreCancelled, KeyStoreDenied};

fn resume_with_key_error(
    fixture: &Fixture,
    reopened: &mut Engine<SyntheticLibraryProvider, impl Fn() -> SyntheticLibraryProvider>,
    at: usize,
    code: SourceVaultErrorCode,
) -> (DeleteRequest, Snapshot) {
    let mut writer = fixture.engine();
    ready(&mut writer);
    ok(writer.execute(Command::Import(
        fixture.input("synthetic persisted deletion"),
    )));
    fixture.provider.interrupt_next_deletion_after_execution();
    assert!(writer.execute(Command::Delete).holds_original);
    let Library::Encrypted {
        deletion: Some(original),
        ..
    } = writer.controller().unwrap().backend()
    else {
        panic!("missing original request");
    };
    let original = original.as_ref().clone();
    drop(writer);

    let id = &original.params().delete_request_id;
    let opened = ok(reopened.execute(Command::OpenEncrypted));
    assert_eq!(opened.deletion_requests, std::slice::from_ref(id));
    let before = fixture.provider.key_loads();
    fixture.provider.fail_on_key_load_with(before + at, code);
    let failed = reopened.execute(Command::ResumeDelete(id.clone()));
    assert_eq!(fixture.provider.key_loads() - before, at, "load {at}");
    assert!(failed.view.is_none() && !failed.recovery_known);
    assert!(!failed.abandonment_available && failed.deletion_requests.is_empty());
    (original, failed)
}

#[test]
fn resume_delete_before_receipt_retains_only_the_original_for_explicit_retry() {
    // Reading the original, executing deletion, and storing the receipt each authenticate.
    for at in 1..=3 {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        let (original, failed) = resume_with_key_error(&fixture, &mut engine, at, KeyStoreDenied);
        let error = failed.error.unwrap();
        assert_eq!(
            error.application_failure().unwrap().vault_code(),
            Some(KeyStoreDenied)
        );
        assert!(
            failed.holds_original
                && failed.refresh_error.is_none()
                && failed.last_deletion.is_none()
        );
        let Library::Encrypted {
            deletion, library, ..
        } = engine.controller().unwrap().backend()
        else {
            panic!("expected encrypted library");
        };
        assert_eq!(deletion.as_deref(), Some(&original));
        assert_eq!(
            library.unfinished_delete_requests().unwrap(),
            std::slice::from_ref(&original)
        );
        let retried = ok(engine.execute(Command::RetryOriginal));
        assert!(!retried.holds_original && retried.refresh_error.is_none());
        assert_eq!(
            retried.last_deletion.unwrap().params().delete_request_id,
            original.params().delete_request_id
        );
    }
}

#[test]
fn resume_delete_after_receipt_reports_refresh_failure_and_never_repeats_the_write() {
    // With no remaining sources, the catalog and the two recovery reads each load once.
    for (at, code) in [
        (4, KeyStoreDenied),
        (5, KeyStoreDenied),
        (6, KeyStoreDenied),
        (4, KeyStoreCancelled),
    ] {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        let (original, failed) = resume_with_key_error(&fixture, &mut engine, at, code);
        assert!(failed.error.is_none() && !failed.holds_original);
        let error = failed.refresh_error.unwrap();
        assert_eq!(
            error.application_failure().unwrap().vault_code(),
            Some(code)
        );
        let evidence = failed.last_deletion.unwrap();
        let id = &original.params().delete_request_id;
        assert_eq!(&evidence.params().delete_request_id, id);
        assert_eq!(
            evidence.params().overall_status,
            DeletionOverallStatus::Completed
        );
        evidence.verify_digest().unwrap();

        let loads = fixture.provider.key_loads();
        assert!(
            engine
                .execute(Command::ResumeDelete(id.clone()))
                .error
                .is_some()
        );
        assert_eq!(fixture.provider.key_loads(), loads);
        let refreshed = ok(engine.execute(Command::Refresh));
        assert!(refreshed.view.is_some() && refreshed.recovery_known);
        assert!(refreshed.deletion_requests.is_empty());
        drop(engine);
        let mut reopened = fixture.engine();
        assert!(
            ok(reopened.execute(Command::OpenEncrypted))
                .deletion_requests
                .is_empty()
        );
        let Library::Encrypted { library, .. } = reopened.controller().unwrap().backend() else {
            panic!("expected encrypted library");
        };
        let stored = library
            .get_deletion_evidence(&evidence.params().deletion_evidence_id)
            .unwrap();
        assert_eq!(stored, Some(evidence));
    }
}
