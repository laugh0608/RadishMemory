use super::*;
use radishmemory_sqlite::RecallProfile;

fn fixture(target: RecallProfile) -> (TestDirectory, ObjectDirectory) {
    let (root, dir) = setup(true);
    if target == RecallProfile::HanPhraseV1 {
        change_profile(&dir, NS, DEVICE, RecallProfile::LegacyUnicode61, || {
            Ok(key())
        })
        .unwrap();
    }
    (root, dir)
}

#[test]
fn recall_transition_checkpoints_preserve_facts_and_report_post_commit_state() {
    for target in [RecallProfile::LegacyUnicode61, RecallProfile::HanPhraseV1] {
        for stop in [
            Step::Authenticated,
            Step::BeforeCommit,
            Step::Committed,
            Step::BeforeReadBack,
        ] {
            let (root, dir) = fixture(target);
            let canonical = snapshot(&root, true);
            let original = snapshot(&root, false);
            let objects = files(&root);
            assert!(
                maintain_action_with_step(
                    &dir,
                    NS,
                    DEVICE,
                    Action::ChangeProfile(target),
                    || Ok(key()),
                    |step| { if step == stop { Err(failure()) } else { Ok(()) } }
                )
                .is_err()
            );
            if matches!(stop, Step::Authenticated | Step::BeforeCommit) {
                assert_eq!(snapshot(&root, false), original);
            } else {
                assert_eq!(check(&dir, false).unwrap().recall_profile, target);
            }
            assert_eq!(snapshot(&root, true), canonical);
            assert_eq!(files(&root), objects);
            assert_eq!(
                change_profile(&dir, NS, DEVICE, target, || Ok(key()))
                    .unwrap()
                    .recall_profile,
                target
            );
        }
    }
}

#[test]
fn recall_transition_reauthenticates_objects_and_excludes_another_writer() {
    for stop in [Step::BeforeCommit, Step::BeforeReadBack] {
        let (root, dir) = fixture(RecallProfile::LegacyUnicode61);
        let path = object(&root);
        let original_bytes = fs::read(&path).unwrap();
        let original_rows = snapshot(&root, false);
        let second = Connection::open(root.0.join("library.sqlite3")).unwrap();
        second.busy_timeout(std::time::Duration::ZERO).unwrap();
        assert!(
            maintain_action_with_step(
                &dir,
                NS,
                DEVICE,
                Action::ChangeProfile(RecallProfile::LegacyUnicode61),
                || Ok(key()),
                |step| {
                    if step == Step::Authenticated {
                        assert!(second.execute_batch("BEGIN IMMEDIATE").is_err());
                    }
                    if step == stop {
                        fs::write(&path, b"Synthetic late ciphertext damage").unwrap();
                    }
                    Ok(())
                }
            )
            .is_err()
        );
        drop(second);
        if stop == Step::BeforeCommit {
            assert_eq!(snapshot(&root, false), original_rows);
        }
        assert!(check(&dir, false).is_err());
        fs::write(path, original_bytes).unwrap();
        let expected = if stop == Step::BeforeCommit {
            RecallProfile::HanPhraseV1
        } else {
            RecallProfile::LegacyUnicode61
        };
        assert_eq!(check(&dir, false).unwrap().recall_profile, expected);
    }
}

#[test]
fn independent_process_exit_recovers_complete_old_or_new_profile() {
    for target in ["legacy", "han"] {
        for stop in ["before-commit", "committed"] {
            let target_profile = if target == "legacy" {
                RecallProfile::LegacyUnicode61
            } else {
                RecallProfile::HanPhraseV1
            };
            let (root, dir) = fixture(target_profile);
            let canonical = snapshot(&root, true);
            let original = snapshot(&root, false);
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "maintenance::tests::recall_profile::recall_transition_child",
                    "--ignored",
                ])
                .env("RADISHMEMORY_RECALL_TEST_ROOT", &root.0)
                .env("RADISHMEMORY_RECALL_TEST_TARGET", target)
                .env("RADISHMEMORY_RECALL_TEST_STEP", stop)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(75),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if stop == "before-commit" {
                assert_eq!(snapshot(&root, false), original);
            } else {
                assert_eq!(check(&dir, false).unwrap().recall_profile, target_profile);
            }
            assert_eq!(snapshot(&root, true), canonical);
            change_profile(&dir, NS, DEVICE, target_profile, || Ok(key())).unwrap();
        }
    }
}

#[test]
#[ignore = "invoked only by parent with an isolated synthetic root"]
fn recall_transition_child() {
    let Some(root) = std::env::var_os("RADISHMEMORY_RECALL_TEST_ROOT") else {
        return;
    };
    let target = match std::env::var("RADISHMEMORY_RECALL_TEST_TARGET")
        .unwrap()
        .as_str()
    {
        "legacy" => RecallProfile::LegacyUnicode61,
        "han" => RecallProfile::HanPhraseV1,
        _ => panic!("unexpected synthetic profile"),
    };
    let stop = std::env::var("RADISHMEMORY_RECALL_TEST_STEP").unwrap();
    let directory = ObjectDirectory::open_application_directory(PathBuf::from(root)).unwrap();
    maintain_action_with_step(
        &directory,
        NS,
        DEVICE,
        Action::ChangeProfile(target),
        || Ok(key()),
        |step| {
            if (stop == "before-commit" && step == Step::BeforeCommit)
                || (stop == "committed" && step == Step::Committed)
            {
                std::process::exit(75);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("synthetic transition checkpoint not reached");
}
