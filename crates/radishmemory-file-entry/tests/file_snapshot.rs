use std::error::Error as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use radishmemory_core::{Identifier, MediaType, SourceKind, Version, compute_exact_bytes_digest};
use radishmemory_file_entry::{
    FILE_ENTRY_CONTRACT_ID, FileCaptureOutcome, FileCaptureReceipt, FileEntryErrorReason,
    FileReadRequest, MAX_FILE_BYTES, read_file_snapshot,
};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "radishmemory-file-entry-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("isolated test directory must be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).expect("isolated test directory must be removed");
    }
}

fn request(path: &Path, root: &Path) -> FileReadRequest {
    FileReadRequest::new(path, vec![root.to_path_buf()]).expect("request must be valid")
}

fn assert_reason(path: &Path, root: &Path, expected: FileEntryErrorReason) {
    let error = read_file_snapshot(&request(path, root)).expect_err("file must be rejected");
    assert_eq!(error.reason(), expected);
}

#[test]
fn snapshot_preserves_markdown_exact_bytes_without_path_or_content_debug() {
    let directory = TestDirectory::new("exact");
    let selected = directory.child("private-title.MD");
    let content = "\u{feff}# Synthetic\r\nCafe\u{301}\r\n";
    fs::write(&selected, content.as_bytes()).expect("synthetic file must be written");

    let read_request = request(&selected, directory.path());
    let snapshot = read_file_snapshot(&read_request).expect("valid Markdown must be read");

    assert_eq!(snapshot.source_kind(), SourceKind::Markdown);
    assert_eq!(snapshot.media_type(), MediaType::TextMarkdown);
    assert_eq!(snapshot.content().as_str().as_bytes(), content.as_bytes());
    assert_eq!(snapshot.content_length(), content.len() as u64);
    assert_eq!(
        snapshot.content_digest(),
        &compute_exact_bytes_digest(content.as_bytes())
    );
    assert_eq!(
        snapshot.title().map(|title| title.as_str()),
        Some("private-title.MD")
    );

    let request_debug = format!("{read_request:?}");
    let snapshot_debug = format!("{snapshot:?}");
    assert!(!request_debug.contains("private-title"));
    assert!(!request_debug.contains(directory.path().to_string_lossy().as_ref()));
    assert!(!snapshot_debug.contains("Synthetic"));
    assert!(!snapshot_debug.contains("private-title"));
}

#[test]
fn allowed_root_and_regular_file_rules_fail_closed() {
    let allowed = TestDirectory::new("allowed");
    let outside = TestDirectory::new("outside");
    let outside_file = outside.child("outside.txt");
    fs::write(&outside_file, b"outside").expect("synthetic outside file must be written");
    assert_reason(
        &outside_file,
        allowed.path(),
        FileEntryErrorReason::PathNotAllowed,
    );

    let directory_named_file = allowed.child("folder.txt");
    fs::create_dir(&directory_named_file).expect("synthetic directory must be created");
    assert_reason(
        &directory_named_file,
        allowed.path(),
        FileEntryErrorReason::NotRegularFile,
    );

    let relative = FileReadRequest::new("relative.txt", vec![allowed.path().to_path_buf()])
        .expect("shape validation does not resolve the path");
    let error = read_file_snapshot(&relative).expect_err("relative path must be rejected");
    assert_eq!(error.reason(), FileEntryErrorReason::PathNotAllowed);

    let error = FileReadRequest::new(allowed.child("missing.txt"), vec![])
        .expect_err("empty allowed roots must be rejected");
    assert_eq!(error.reason(), FileEntryErrorReason::PathNotAllowed);
}

#[cfg(unix)]
#[test]
fn symlink_below_allowed_root_is_never_followed() {
    use std::os::unix::fs::symlink;

    let allowed = TestDirectory::new("symlink-allowed");
    let outside = TestDirectory::new("symlink-outside");
    let outside_file = outside.child("target.txt");
    fs::write(&outside_file, b"outside target").expect("synthetic target must be written");
    let link = allowed.child("linked.txt");
    symlink(&outside_file, &link).expect("synthetic symlink must be created");

    assert_reason(
        &link,
        allowed.path(),
        FileEntryErrorReason::SymlinkNotAllowed,
    );
}

#[test]
fn extension_utf8_nul_and_empty_rules_return_distinct_reasons() {
    let directory = TestDirectory::new("content-rejections");
    let unsupported = directory.child("source.markdown");
    fs::write(&unsupported, b"text").expect("synthetic file must be written");
    assert_reason(
        &unsupported,
        directory.path(),
        FileEntryErrorReason::UnsupportedFileType,
    );

    let empty = directory.child("empty.txt");
    fs::write(&empty, []).expect("synthetic empty file must be written");
    assert_reason(&empty, directory.path(), FileEntryErrorReason::EmptyFile);

    let invalid_utf8 = directory.child("invalid.txt");
    fs::write(&invalid_utf8, [0xff, 0xfe]).expect("synthetic bytes must be written");
    let invalid_error =
        read_file_snapshot(&request(&invalid_utf8, directory.path())).expect_err("must reject");
    assert_eq!(invalid_error.reason(), FileEntryErrorReason::InvalidUtf8);
    assert!(invalid_error.source().is_none());

    let nul = directory.child("nul.txt");
    fs::write(&nul, b"before\0after").expect("synthetic bytes must be written");
    assert_reason(
        &nul,
        directory.path(),
        FileEntryErrorReason::NulByteNotAllowed,
    );
}

#[test]
fn byte_limit_accepts_exact_boundary_and_rejects_one_more_byte() {
    let directory = TestDirectory::new("size-boundary");
    let exact = directory.child("exact.txt");
    fs::write(&exact, vec![b'x'; MAX_FILE_BYTES as usize])
        .expect("exact-boundary file must be written");
    let snapshot = read_file_snapshot(&request(&exact, directory.path()))
        .expect("exact-boundary file must be accepted");
    assert_eq!(snapshot.content_length(), MAX_FILE_BYTES);

    let oversized = directory.child("oversized.txt");
    fs::write(&oversized, vec![b'x'; MAX_FILE_BYTES as usize + 1])
        .expect("oversized file must be written");
    assert_reason(
        &oversized,
        directory.path(),
        FileEntryErrorReason::FileTooLarge,
    );
}

#[test]
fn hardlink_aliases_are_read_as_files_without_becoming_snapshot_identity() {
    let directory = TestDirectory::new("hardlink");
    let first = directory.child("first.txt");
    let alias = directory.child("alias.txt");
    fs::write(&first, b"same managed bytes").expect("synthetic source must be written");
    fs::hard_link(&first, &alias).expect("synthetic hardlink must be created");

    let first_snapshot =
        read_file_snapshot(&request(&first, directory.path())).expect("first path must read");
    let alias_snapshot =
        read_file_snapshot(&request(&alias, directory.path())).expect("alias path must read");
    assert_eq!(
        first_snapshot.content_digest(),
        alias_snapshot.content_digest()
    );
    assert_eq!(first_snapshot.title().unwrap().as_str(), "first.txt");
    assert_eq!(alias_snapshot.title().unwrap().as_str(), "alias.txt");
}

#[test]
fn receipt_has_fixed_contract_and_no_path_or_content_field() {
    let digest = compute_exact_bytes_digest(b"receipt bytes");
    let receipt = FileCaptureReceipt::new(
        Identifier::new("namespace-1").unwrap(),
        Identifier::new("source-1").unwrap(),
        Identifier::new("lineage-1").unwrap(),
        Version::new(1).unwrap(),
        digest.clone(),
        13,
        MediaType::TextPlain,
        FileCaptureOutcome::Created,
    )
    .expect("receipt must be valid");

    assert_eq!(receipt.contract_id(), FILE_ENTRY_CONTRACT_ID);
    assert_eq!(receipt.content_digest(), &digest);
    assert_eq!(receipt.outcome(), FileCaptureOutcome::Created);
    let debug = format!("{receipt:?}");
    assert!(!debug.contains("receipt bytes"));
    assert!(!debug.contains("private-path"));
    assert!(!debug.contains("selected_path"));
}

#[test]
fn rejected_content_never_appears_in_error_or_debug() {
    let directory = TestDirectory::new("redaction");
    let selected = directory.child("private-path.txt");
    fs::write(&selected, [0xff, b's', b'e', b'c', b'r', b'e', b't'])
        .expect("synthetic invalid content must be written");

    let error = read_file_snapshot(&request(&selected, directory.path()))
        .expect_err("invalid UTF-8 must be rejected");
    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(!rendered.contains("private-path"));
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains(directory.path().to_string_lossy().as_ref()));
    }
}
