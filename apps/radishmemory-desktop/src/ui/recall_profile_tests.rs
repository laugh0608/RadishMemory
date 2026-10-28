//! Real worker transitions behind headless confirmation, including late failure.
use super::{test_support::Harness, *};
use crate::worker::tests::{Fixture, ok, ready};
use radishmemory_application::RecallProfile;
use radishmemory_source_vault::SourceVaultErrorCode;

#[test]
fn recall_format_confirmation_cancel_and_confirm_dispatch_exactly_once() {
    for encrypted in [false, true] {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        if encrypted {
            ready(&mut engine);
        } else {
            ok(engine.execute(Command::OpenPlain));
        }
        ok(engine.execute(Command::Import(fixture.input("项目默认使用蓝色主题。"))));
        let populated = ok(engine.execute(Command::Search("蓝色".into())));
        assert_eq!(populated.view.as_ref().unwrap().search_results().len(), 1);
        let mut h = Harness::new();
        h.results.send(populated).unwrap();
        h.frame(false, vec![]).drop_without_applying_deltas();
        let loads = fixture.provider.key_loads();
        h.click("Use legacy search…").drop_without_applying_deltas();
        h.no_command();
        h.click("Cancel").drop_without_applying_deltas();
        h.no_command();
        assert_eq!(fixture.provider.key_loads(), loads);
        h.click("Use legacy search…").drop_without_applying_deltas();
        h.click("Confirm").drop_without_applying_deltas();
        let command = h.commands.try_recv().unwrap();
        assert!(matches!(
            command,
            Command::ChangeRecallProfile(RecallProfile::LegacyUnicode61)
        ));
        h.no_command();
        assert!(h.app.busy());
        let changed = ok(engine.execute(command));
        assert_eq!(changed.recall_profile, Some(RecallProfile::LegacyUnicode61));
        assert!(changed.view.as_ref().unwrap().search_results().is_empty());
        h.results.send(changed).unwrap();
        h.frame(false, vec![]).drop_without_applying_deltas();
        h.click("Enable Chinese phrase search…")
            .drop_without_applying_deltas();
        h.click("Confirm").drop_without_applying_deltas();
        let command = h.commands.try_recv().unwrap();
        assert!(matches!(
            command,
            Command::ChangeRecallProfile(RecallProfile::HanPhraseV1)
        ));
        let restored = ok(engine.execute(command));
        assert_eq!(restored.recall_profile, Some(RecallProfile::HanPhraseV1));
        assert_eq!(
            ok(engine.execute(Command::Search("蓝色".into())))
                .view
                .unwrap()
                .search_results()
                .len(),
            1
        );
    }
}

#[test]
fn recall_change_failure_withdraws_views_and_refresh_reports_actual_commit_state() {
    for after_commit in [false, true] {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        ready(&mut engine);
        ok(engine.execute(Command::Import(fixture.input("项目默认使用蓝色主题。"))));
        ok(engine.execute(Command::Search("蓝色".into())));
        let loads = fixture.provider.key_loads();
        fixture.provider.fail_on_key_load_with(
            loads + if after_commit { 2 } else { 1 },
            SourceVaultErrorCode::KeyStoreLocked,
        );
        let failed = engine.execute(Command::ChangeRecallProfile(RecallProfile::LegacyUnicode61));
        if after_commit {
            assert!(failed.error.is_none());
            assert!(failed.refresh_error.is_some());
        } else {
            assert!(failed.error.is_some());
        }
        assert!(failed.view.is_none());
        assert!(failed.recall_profile.is_none());
        let recovered = ok(engine.execute(Command::Refresh));
        assert_eq!(
            recovered.recall_profile,
            Some(if after_commit {
                RecallProfile::LegacyUnicode61
            } else {
                RecallProfile::HanPhraseV1
            })
        );
        assert!(recovered.view.unwrap().search_results().is_empty());
        assert_eq!(fixture.provider.key_creations(), 1);
    }
}

#[test]
fn pending_original_request_blocks_recall_change_before_provider_access() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    fixture.provider.interrupt_next_capture_after_publish();
    let pending = engine.execute(Command::Import(fixture.input("合成蓝色主题。")));
    assert!(pending.error.is_some());
    assert!(pending.holds_original);
    let loads = fixture.provider.key_loads();
    assert!(
        engine
            .execute(Command::ChangeRecallProfile(RecallProfile::LegacyUnicode61))
            .error
            .is_some()
    );
    assert_eq!(fixture.provider.key_loads(), loads);
    ok(engine.execute(Command::RetryOriginal));
    assert_eq!(
        ok(engine.execute(Command::ChangeRecallProfile(RecallProfile::LegacyUnicode61)))
            .recall_profile,
        Some(RecallProfile::LegacyUnicode61)
    );
}
