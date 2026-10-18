use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

const NAMESPACE: &str = "namespace-0123456789abcdef0123456789abcdef";
const DEVICE: &str = "device-fedcba9876543210fedcba9876543210";
static NEXT: AtomicU64 = AtomicU64::new(1);

pub(crate) struct Fixture {
    pub(crate) root: PathBuf,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "radishmemory-native-startup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        crate::ApplicationPaths::from_data_directory(&root)
            .unwrap()
            .prepare()
            .unwrap();
        fs::write(
            root.join("host-profile-v1.txt"),
            format!(
                "contract_id={}\nnamespace_id={NAMESPACE}\ndevice_id={DEVICE}\n",
                crate::HOST_PROFILE_CONTRACT_ID
            ),
        )
        .unwrap();
        Self { root }
    }

    pub(crate) fn arguments(&self) -> Vec<OsString> {
        vec![
            "--acceptance-root".into(),
            self.root.clone().into_os_string(),
            "--acceptance-namespace".into(),
            NAMESPACE.into(),
            "--acceptance-device".into(),
            DEVICE.into(),
        ]
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn default_and_valid_acceptance_parse_without_writes() {
    assert!(matches!(Startup::parse([]).unwrap(), Startup::Default));
    let fixture = Fixture::new();
    let startup = Startup::parse(fixture.arguments()).unwrap();
    let Startup::Acceptance(location) = startup else {
        panic!("wrong mode")
    };
    assert_eq!(location.paths().unwrap().data_directory(), fixture.root);
    assert_eq!(
        location.load_profile().unwrap().namespace_id().as_str(),
        NAMESPACE
    );
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 1);
}

#[test]
fn invalid_arguments_never_fall_back_or_echo_input() {
    let fixture = Fixture::new();
    let valid = fixture.arguments();
    let mut cases = vec![
        vec!["--unknown-synthetic-private-marker".into()],
        valid[..2].to_vec(),
        valid[..4].to_vec(),
        valid[..5].to_vec(),
    ];
    let mut duplicate = valid.clone();
    duplicate.extend(valid[..2].iter().cloned());
    cases.push(duplicate);
    let mut invalid_id = valid;
    invalid_id[3] = "namespace-SYNTHETIC-private-marker".into();
    cases.push(invalid_id);
    for arguments in cases {
        let error = Startup::parse(arguments).err().unwrap();
        assert_eq!(error.reason(), DesktopErrorReason::StartupArgumentsInvalid);
        assert!(!format!("{error:?} {error}").contains("private-marker"));
    }
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 1);
}

#[test]
fn profile_is_required_and_identity_is_pinned_without_repair() {
    let fixture = Fixture::new();
    let original = fs::read(fixture.root.join("host-profile-v1.txt")).unwrap();
    let Startup::Acceptance(location) = Startup::parse(fixture.arguments()).unwrap() else {
        panic!("wrong mode")
    };
    let profile_path = fixture.root.join("host-profile-v1.txt");
    let different = String::from_utf8(original.clone())
        .unwrap()
        .replace(NAMESPACE, "namespace-00000000000000000000000000000000");
    fs::write(&profile_path, &different).unwrap();
    assert_eq!(
        location.load_profile().unwrap_err().reason(),
        DesktopErrorReason::AcceptanceProfileMismatch
    );
    assert_eq!(fs::read_to_string(&profile_path).unwrap(), different);
    fs::remove_file(&profile_path).unwrap();
    assert_eq!(
        location.load_profile().unwrap_err().reason(),
        DesktopErrorReason::ProfileReadFailed
    );
    assert!(!profile_path.exists());
    fs::write(&profile_path, b"synthetic-invalid-profile").unwrap();
    assert_eq!(
        location.load_profile().unwrap_err().reason(),
        DesktopErrorReason::ProfileInvalid
    );
    fs::write(&profile_path, original).unwrap();
    assert!(location.load_profile().is_ok());
}

#[test]
fn root_must_exist_and_cannot_overlap_default_or_traverse_parents() {
    let fixture = Fixture::new();
    let default = fixture.root.join("default");
    fs::create_dir(&default).unwrap();
    let isolated = fixture.root.join("isolated");
    fs::create_dir(&isolated).unwrap();
    crate::ApplicationPaths::from_data_directory(&isolated)
        .unwrap()
        .prepare()
        .unwrap();
    assert!(validate_root(&isolated, &default).is_ok());
    // Windows canonical paths have a verbatim prefix; PathBuf::join normalizes
    // parent components there. Preserve the raw argument so this tests rejection.
    let mut traversal = isolated.as_os_str().to_owned();
    traversal.push(std::path::MAIN_SEPARATOR_STR);
    traversal.push("..");
    traversal.push(std::path::MAIN_SEPARATOR_STR);
    traversal.push("isolated");
    let traversal = PathBuf::from(traversal);
    assert!(
        traversal
            .components()
            .any(|component| component == Component::ParentDir)
    );
    for root in [
        PathBuf::from("relative"),
        fixture.root.clone(),
        default.clone(),
        default.join("child"),
        isolated.join("missing"),
        traversal,
        fixture.root.join("host-profile-v1.txt"),
    ] {
        assert_eq!(
            validate_root(&root, &default).unwrap_err().reason(),
            DesktopErrorReason::DataDirectoryInvalid
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_root_ancestor_and_profile_are_rejected() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let default = fixture.root.join("default");
    fs::create_dir(fixture.root.join("nested")).unwrap();
    symlink(&fixture.root, fixture.root.join("alias")).unwrap();
    assert!(validate_root(&fixture.root.join("alias"), &default).is_err());
    assert!(validate_root(&fixture.root.join("alias/nested"), &default).is_err());
    assert!(validate_root(&fixture.root, &fixture.root.join("alias/not-created")).is_err());
    let original = fixture.root.join("host-profile-v1.txt");
    fs::rename(&original, fixture.root.join("original-profile")).unwrap();
    symlink(fixture.root.join("original-profile"), original).unwrap();
    assert!(Startup::parse(fixture.arguments()).is_err());
}

#[cfg(unix)]
#[test]
fn loose_permissions_and_symlink_database_fail_before_library_open() {
    use std::os::unix::{fs::PermissionsExt, fs::symlink};
    let fixture = Fixture::new();
    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        Startup::parse(fixture.arguments()).err().unwrap().reason(),
        DesktopErrorReason::DataDirectoryInvalid
    );
    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(
        fixture.root.join("host-profile-v1.txt"),
        fixture.root.join("library.sqlite3"),
    )
    .unwrap();
    assert_eq!(
        Startup::parse(fixture.arguments()).err().unwrap().reason(),
        DesktopErrorReason::DatabasePathInvalid
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_identifier_is_rejected_without_echo() {
    use std::os::unix::ffi::OsStringExt;
    let fixture = Fixture::new();
    let mut arguments = fixture.arguments();
    arguments[5] = OsString::from_vec(vec![0xff]);
    assert_eq!(
        Startup::parse(arguments).err().unwrap().reason(),
        DesktopErrorReason::StartupArgumentsInvalid
    );
}

#[test]
fn trace_requires_complete_unique_arguments_and_supported_platform_without_creation() {
    let fixture = Fixture::new();
    let output = Fixture::new();
    let path = output.root.join("trace");
    fs::write(
        fixture.root.join("library.sqlite3"),
        b"synthetic-existing-db",
    )
    .unwrap();
    let mut args = fixture.arguments();
    args.extend([
        "--acceptance-key-read-trace".into(),
        path.as_os_str().to_owned(),
    ]);
    if cfg!(target_os = "macos") {
        let parsed = Startup::parse(args.clone()).unwrap();
        let Startup::Acceptance(location) = parsed else {
            panic!("wrong mode")
        };
        assert!(location.trace.is_some());
    } else {
        assert!(Startup::parse(args.clone()).is_err());
    }
    assert!(!path.exists());
    args.extend([
        "--acceptance-key-read-trace".into(),
        path.as_os_str().to_owned(),
    ]);
    assert!(Startup::parse(args).is_err());
    for args in [
        vec![
            "--acceptance-key-read-trace".into(),
            path.as_os_str().to_owned(),
        ],
        vec!["--acceptance-key-read-trace".into()],
    ] {
        assert!(Startup::parse(args).is_err());
    }
    let mut args = fixture.arguments();
    args.extend([
        "--acceptance-key-read-trace".into(),
        fixture.root.join("inside-vault").into_os_string(),
    ]);
    let error = Startup::parse(args).err().unwrap();
    assert!(!format!("{error:?} {error}").contains("inside-vault"));
    assert!(!path.exists());
}

#[cfg(target_os = "macos")]
#[test]
fn diagnostic_mode_requires_existing_database_without_creating_it() {
    let fixture = Fixture::new();
    let output = Fixture::new();
    let path = output.root.join("trace");
    let mut args = fixture.arguments();
    args.extend([
        "--acceptance-key-read-trace".into(),
        path.as_os_str().to_owned(),
    ]);
    assert_eq!(
        Startup::parse(args).err().unwrap().reason(),
        DesktopErrorReason::KeyReadObservationInvalid
    );
    assert!(!fixture.root.join("library.sqlite3").exists());
    assert!(!path.exists());
}
