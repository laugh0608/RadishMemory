use super::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
pub(crate) struct Recording {
    pub(crate) bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
    fail_write: Option<usize>,
    fail_flush: Option<usize>,
}
struct Sink(Rc<RefCell<Recording>>);
impl EventSink for Sink {
    fn emit(&mut self, event: &[u8]) -> io::Result<()> {
        let mut log = self.0.borrow_mut();
        log.writes += 1;
        if log.fail_write == Some(log.writes) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        log.bytes.extend_from_slice(event);
        log.flushes += 1;
        if log.fail_flush == Some(log.flushes) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        Ok(())
    }
}
pub(crate) fn recording<T, E>(
    fail_write: Option<usize>,
    fail_flush: Option<usize>,
    work: impl FnOnce() -> Result<T, E>,
) -> (Result<T, TraceFailure<E>>, Rc<RefCell<Recording>>) {
    let log = Rc::new(RefCell::new(Recording {
        fail_write,
        fail_flush,
        ..Recording::default()
    }));
    (with_sink(Box::new(Sink(log.clone())), work), log)
}
#[test]
fn pairing_ids_fixed_fields_and_no_sensitive_values() {
    let (result, log) = recording(None, None, || {
        call(Operation::Entry, || {
            call(Operation::Read1, || {
                call(Operation::SecretSearch, || Ok("synthetic-private-marker"))
            })
        })
    });
    result.unwrap();
    let text = String::from_utf8(log.borrow().bytes.clone()).unwrap();
    assert_eq!(text.lines().count(), 8);
    assert!(text.contains("\"call\":4,\"parent\":3,\"stage\":\"entry\",\"read\":1"));
    assert!(!text.contains("synthetic"));
    assert!(text.ends_with("\"result\":\"ok\"}\n"));
    for (index, line) in text.lines().enumerate() {
        assert!(line.contains(&format!("\"seq\":{},", index + 1)));
    }
}
#[test]
fn every_write_and_flush_failure_blocks_later_calls_and_preserves_business_error() {
    for flush in [false, true] {
        for at in 1..=4 {
            let calls = Cell::new(0);
            let (result, _) = recording((!flush).then_some(at), flush.then_some(at), || {
                let first = call(Operation::SecretSearch, || {
                    calls.set(calls.get() + 1);
                    Ok(())
                });
                if first.is_err() {
                    assert!(
                        call(Operation::Domain, || {
                            calls.set(calls.get() + 1);
                            Ok(())
                        })
                        .is_err()
                    );
                }
                first
            });
            assert!(result.is_err());
            assert_eq!(calls.get(), usize::from(at >= 3));
        }
    }
    let (result, _) = recording(Some(3), None, || {
        call(Operation::Domain, || {
            Err::<(), _>(SourceVaultError::provider(
                SourceVaultErrorCode::KeyStoreDenied,
                "synthetic-private-marker",
                Some(-999),
            ))
        })
    });
    let Err(TraceFailure::Business(error)) = result else {
        panic!("business error lost")
    };
    assert_eq!(error.code(), SourceVaultErrorCode::KeyStoreDenied);
}
#[test]
fn nested_scope_unwind_pairing_and_limits_poison_without_cross_action_or_thread_state() {
    let (result, log) = recording(None, None, || {
        assert!(recording(None, None, || Ok::<_, ()>(())).0.is_err());
        assert!(call::<()>(Operation::Domain, || panic!("must not call")).is_err());
        Ok::<_, ()>(())
    });
    assert!(result.is_err());
    assert_eq!(
        String::from_utf8(log.borrow().bytes.clone())
            .unwrap()
            .lines()
            .count(),
        1
    );
    let (result, _) = recording(None, None, || {
        assert!(
            std::panic::catch_unwind(|| {
                let _ = call::<()>(Operation::Domain, || panic!("synthetic unwind"));
            })
            .is_err()
        );
        assert!(call::<()>(Operation::Domain, || panic!("must not call")).is_err());
        Ok::<_, ()>(())
    });
    assert!(result.is_err());
    let unwind = std::panic::catch_unwind(|| {
        let _ = recording(None, None, || -> Result<(), ()> { panic!("scope unwind") });
    });
    assert!(unwind.is_err());
    recording(None, None, || Ok::<_, ()>(())).0.unwrap();
    let (thread_result, thread_log) = recording(None, None, || {
        std::thread::spawn(|| call(Operation::Domain, || Ok(())))
            .join()
            .unwrap()
            .unwrap();
        Ok::<_, ()>(())
    });
    thread_result.unwrap();
    assert_eq!(
        String::from_utf8(thread_log.borrow().bytes.clone())
            .unwrap()
            .lines()
            .count(),
        2
    );
    for bytes_limit in [false, true] {
        let (result, _) = recording(None, None, || {
            ACTIVE.with(|active| {
                let mut active = active.borrow_mut();
                let c = active.as_mut().unwrap();
                if bytes_limit {
                    c.bytes = 64 * 1024;
                } else {
                    c.seq = 256;
                }
            });
            assert!(call::<()>(Operation::Domain, || panic!("limit must stop call")).is_err());
            Ok::<_, ()>(())
        });
        assert!(result.is_err());
    }
    let (result, _) = recording(None, None, || {
        ACTIVE.with(|active| {
            active
                .borrow_mut()
                .as_mut()
                .unwrap()
                .end(999, Operation::Domain, "ok")
                .unwrap_err();
        });
        assert!(call::<()>(Operation::Domain, || panic!("bad pair must stop call")).is_err());
        Ok::<_, ()>(())
    });
    assert!(result.is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn file_is_new_private_exclusive_and_pinned() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("radishmemory-trace-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let nested_path = root.join("nested");
    let outer = TraceFile::validate(&root.join("outer"), &[]).unwrap();
    let nested = TraceFile::validate(&nested_path, &[]).unwrap();
    assert!(
        with_trace(outer, || {
            assert!(with_trace(nested, || Ok::<_, ()>(())).is_err());
            Ok::<_, ()>(())
        })
        .is_err()
    );
    assert!(!nested_path.exists());
    let path = root.join("trace");
    assert!(TraceFile::validate(&path, &[&root]).is_err());
    let config = TraceFile::validate(&path, &[]).unwrap();
    assert!(!path.exists());
    with_trace(config, || Ok::<_, ()>(())).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(TraceFile::validate(&path, &[]).is_err());
    let alias = root.join("alias");
    symlink(&root, &alias).unwrap();
    assert!(TraceFile::validate(&alias.join("new"), &[]).is_err());
    let new = root.join("new");
    let config = TraceFile::validate(&new, &[]).unwrap();
    std::fs::write(&new, b"synthetic-existing").unwrap();
    assert!(with_trace::<(), ()>(config, || panic!("must not run")).is_err());
    let changed = root.join("changed");
    let config = TraceFile::validate(&changed, &[]).unwrap();
    let (result, ()) = (
        with_trace(config, || {
            std::fs::write(&changed, b"synthetic-tamper").unwrap();
            Ok::<_, ()>(())
        }),
        (),
    );
    assert!(result.is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn file_rejects_ancestor_replacement_leaf_aliases_permissions_links_and_tampering() {
    use std::io::Write;
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("radishmemory-trace-paths-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    for path in [
        std::path::PathBuf::from("relative"),
        root.join("../new"),
        root.join("missing/new"),
    ] {
        assert!(TraceFile::validate(&path, &[]).is_err());
    }
    let existing = root.join("existing");
    std::fs::write(&existing, b"synthetic-existing").unwrap();
    let leaf = root.join("alias");
    symlink(&existing, &leaf).unwrap();
    assert!(TraceFile::validate(&leaf, &[]).is_err());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(TraceFile::validate(&root.join("loose"), &[]).is_err());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let parent = root.join("parent");
    std::fs::create_dir(&parent).unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let config = TraceFile::validate(&parent.join("trace"), &[]).unwrap();
    std::fs::rename(&parent, root.join("old-parent")).unwrap();
    std::fs::create_dir(&parent).unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(with_trace::<(), ()>(config, || panic!("changed parent must stop action")).is_err());
    assert!(!parent.join("trace").exists());
    for case in 0..6 {
        let path = root.join(format!("trace-{case}"));
        let config = TraceFile::validate(&path, &[]).unwrap();
        let called = Cell::new(false);
        let result = with_trace(config, || {
            match case {
                0 => {
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap()
                }
                1 => std::fs::hard_link(&path, root.join("hardlink")).unwrap(),
                2 => {
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open(&path)
                        .unwrap()
                        .write_all(b"synthetic-tamper")
                        .unwrap();
                }
                3 => {
                    let bytes = std::fs::read(&path).unwrap();
                    std::fs::write(&path, vec![b'X'; bytes.len()]).unwrap();
                }
                4 => {
                    std::fs::rename(&path, root.join("old-trace")).unwrap();
                    std::fs::write(&path, b"replacement").unwrap();
                }
                _ => {
                    std::fs::rename(&path, root.join("symlink-target")).unwrap();
                    symlink(root.join("symlink-target"), &path).unwrap();
                }
            }
            call(Operation::Domain, || {
                called.set(true);
                Ok(())
            })
        });
        assert!(result.is_err(), "case {case}");
        assert!(!called.get(), "case {case}");
    }
    std::fs::remove_dir_all(root).unwrap();
}
