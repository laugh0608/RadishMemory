//! Filesystem publication only: this module never commits canonical facts or deletes orphans.
use std::fmt;
use std::fs;
use std::io::Write;

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::envelope;
use crate::filesystem_support::{self as support, Directory};
use crate::{
    KeyEncryptionKey, ObjectMetadata, SealedObject, SourceVaultError, SourceVaultErrorCode,
    open_object, seal_object,
};

type Result<T> = std::result::Result<T, SourceVaultError>;

/// Adapter-private reference token, never a canonical identity, user path or logging field.
#[derive(Clone, Eq, PartialEq)]
pub struct ObjectLocator(String);
impl ObjectLocator {
    pub fn from_token(token: &str) -> Result<Self> {
        validate_token(token)?;
        Ok(Self(token.to_owned()))
    }
    /// For private reference persistence only. Do not put this value in receipts or diagnostics.
    pub fn token(&self) -> &str {
        &self.0
    }
    fn for_metadata(metadata: &ObjectMetadata) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"radishmemory.object-locator/1\0");
        for value in [
            metadata.namespace_id().as_bytes(),
            metadata.source_id().as_bytes(),
            metadata.exact_digest(),
        ] {
            digest.update((value.len() as u32).to_be_bytes());
            digest.update(value);
        }
        Self(hex(&digest.finalize()))
    }
    fn verify(&self, metadata: &ObjectMetadata) -> Result<()> {
        if *self != Self::for_metadata(metadata) {
            return Err(SourceVaultError::new(
                SourceVaultErrorCode::InvalidLocator,
                "object locator does not match expected source",
            ));
        }
        Ok(())
    }
}

/// Identifies one sealing attempt through its authenticated stream nonce, without exposing that nonce.
#[derive(Clone, Eq, PartialEq)]
pub struct AttemptId(String);
impl AttemptId {
    pub fn from_token(token: &str) -> Result<Self> {
        validate_token(token)?;
        Ok(Self(token.to_owned()))
    }
    /// For private attempt persistence only, before publication. Not a recovery/deletion authorization.
    pub fn token(&self) -> &str {
        &self.0
    }
    fn for_sealed(locator: &ObjectLocator, sealed: &SealedObject) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"radishmemory.object-attempt/1\0");
        digest.update(locator.token().as_bytes());
        digest.update(sealed.stream_nonce_prefix());
        Self(hex(&digest.finalize()))
    }
}

/// Prepared entirely in memory. Contains only metadata and encrypted bytes, never the KEK or plaintext.
pub struct ObjectWrite {
    metadata: ObjectMetadata,
    locator: ObjectLocator,
    attempt: AttemptId,
    envelope: Vec<u8>,
}
impl ObjectWrite {
    pub fn seal(
        key: &KeyEncryptionKey,
        metadata: &ObjectMetadata,
        plaintext: &[u8],
    ) -> Result<Self> {
        envelope::validate_metadata(metadata)?;
        Self::from_sealed(metadata, seal_object(key, metadata, plaintext)?)
    }
    fn from_sealed(metadata: &ObjectMetadata, sealed: SealedObject) -> Result<Self> {
        let locator = ObjectLocator::for_metadata(metadata);
        Ok(Self {
            metadata: metadata.clone(),
            attempt: AttemptId::for_sealed(&locator, &sealed),
            locator,
            envelope: envelope::encode(metadata, &sealed)?,
        })
    }
    pub fn locator(&self) -> &ObjectLocator {
        &self.locator
    }
    pub fn attempt_id(&self) -> &AttemptId {
        &self.attempt
    }
}

/// Filesystem evidence only. SQLite must still commit and independently read back its reference.
pub struct PublishedObject {
    locator: ObjectLocator,
    attempt: AttemptId,
}
impl PublishedObject {
    pub fn locator(&self) -> &ObjectLocator {
        &self.locator
    }
    pub fn attempt_id(&self) -> &AttemptId {
        &self.attempt
    }
}

/// Exact attempt observations, not proof of orphanhood or permission to delete anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptState {
    Absent,
    AuthenticatedStaging,
    AuthenticatedPublishedCandidate,
    AuthenticatedStagingAndPublishedCandidate,
}

pub struct ObjectDirectory {
    root: Directory,
    objects: Directory,
    staging: Directory,
}
impl ObjectDirectory {
    /// The trusted platform caller must resolve and prepare a dedicated application data root.
    /// No default root is chosen. Tests supply an isolated synthetic directory.
    pub fn open_application_directory(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let root = Directory::open(support::application_root(path.as_ref())?)?;
        // Probe directory durability before creating children; unsupported filesystems fail closed.
        root.sync()?;
        let objects = root.child("source-objects-v1")?;
        let staging = root.child("source-staging-v1")?;
        let result = Self {
            root,
            objects,
            staging,
        };
        result.verify()?;
        Ok(result)
    }

    pub fn publish(&self, write: &ObjectWrite, key: &KeyEncryptionKey) -> Result<PublishedObject> {
        self.publish_with_step(write, key, |_| Ok(()))
    }

    fn publish_with_step(
        &self,
        write: &ObjectWrite,
        key: &KeyEncryptionKey,
        step: impl FnMut(Step) -> Result<()>,
    ) -> Result<PublishedObject> {
        self.publish_with_operations(write, key, step, |file, bytes| file.write_all(bytes))
    }

    fn publish_with_operations(
        &self,
        write: &ObjectWrite,
        key: &KeyEncryptionKey,
        mut step: impl FnMut(Step) -> Result<()>,
        write_bytes: impl FnOnce(&mut fs::File, &[u8]) -> std::io::Result<()>,
    ) -> Result<PublishedObject> {
        self.verify()?;
        write.locator.verify(&write.metadata)?;
        // Validate before any disk write, including rejecting the wrong supplied key.
        let sealed = envelope::decode(&write.envelope, &write.metadata)?;
        drop(Zeroizing::new(open_object(key, &write.metadata, &sealed)?));
        let target = self.object_path(&write.locator);
        if support::present(&target)? {
            return Err(support::exists());
        }
        let staging = self.staging_path(&write.locator, &write.attempt);
        step(Step::Create)?;
        self.verify()?;
        let mut file = support::create_new(&staging)?;
        step(Step::Write)?;
        self.verify()?;
        write_bytes(&mut file, &write.envelope)
            .map_err(|e| SourceVaultError::io("write encrypted staging object", e))?;
        step(Step::Flush)?;
        file.flush()
            .map_err(|e| SourceVaultError::io("flush encrypted staging object", e))?;
        step(Step::FileSync)?;
        file.sync_all()
            .map_err(|e| SourceVaultError::io("sync encrypted staging object", e))?;
        let written = support::Observation::of(
            &file
                .metadata()
                .map_err(|e| SourceVaultError::io("inspect written staging object", e))?,
        )?;
        drop(file);
        step(Step::StagingSync)?;
        self.verify()?;
        self.staging.sync()?;
        step(Step::StagingReadBack)?;
        self.verify()?;
        let (bytes, observed) = support::read_file(&staging)?;
        if bytes != write.envelope || observed != written {
            return Err(support::changed());
        }
        self.authenticate(&bytes, &write.metadata, &write.locator, &write.attempt, key)?;
        step(Step::Publish)?;
        self.verify()?;
        support::verify_file(&staging, &observed)?;
        // hard_link creates the final directory entry atomically and never replaces an existing name.
        // Both names are under the same application root. Failure leaves this precise attempt intact.
        fs::hard_link(&staging, &target).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                support::exists()
            } else {
                SourceVaultError::io("publish immutable encrypted object", e)
            }
        })?;
        step(Step::ObjectDirectorySync)?;
        self.verify()?;
        self.objects.sync()?;
        step(Step::PublishedReadBack)?;
        self.verify()?;
        let (published_bytes, published_observation) = support::read_file(&target)?;
        if published_bytes != bytes || published_observation != observed {
            return Err(support::changed());
        }
        self.authenticate(
            &published_bytes,
            &write.metadata,
            &write.locator,
            &write.attempt,
            key,
        )?;
        step(Step::RemoveStaging)?;
        self.verify()?;
        support::verify_file(&staging, &observed)?;
        support::verify_file(&target, &published_observation)?;
        // Only successful publication removes its own verified staging link. No failure-path cleanup.
        fs::remove_file(&staging)
            .map_err(|e| SourceVaultError::io("remove published staging link", e))?;
        step(Step::CleanupSync)?;
        self.staging.sync()?;
        self.verify()?;
        support::verify_file(&target, &published_observation)?;
        Ok(PublishedObject {
            locator: write.locator.clone(),
            attempt: write.attempt.clone(),
        })
    }

    pub fn read(
        &self,
        locator: &ObjectLocator,
        expected: &ObjectMetadata,
        key: &KeyEncryptionKey,
    ) -> Result<Vec<u8>> {
        self.verify()?;
        envelope::validate_metadata(expected)?;
        locator.verify(expected)?;
        let (bytes, observation) = support::read_file(&self.object_path(locator))?;
        let sealed = envelope::decode(&bytes, expected)?;
        let mut plaintext = Zeroizing::new(open_object(key, expected, &sealed)?);
        self.verify()?;
        support::verify_file(&self.object_path(locator), &observation)?;
        Ok(std::mem::take(&mut *plaintext))
    }

    /// Inspect only exact supplied attempt paths. Unknown, incomplete, substituted or conflicting
    /// objects return errors and are retained. Canonical references and recovery belong to P1-S04.
    pub fn inspect_attempt(
        &self,
        locator: &ObjectLocator,
        attempt: &AttemptId,
        expected: &ObjectMetadata,
        key: &KeyEncryptionKey,
    ) -> Result<AttemptState> {
        self.verify()?;
        envelope::validate_metadata(expected)?;
        locator.verify(expected)?;
        let paths = [
            self.staging_path(locator, attempt),
            self.object_path(locator),
        ];
        let mut present = [false; 2];
        let mut observations = [None, None];
        let mut contents = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            if support::present(path)? {
                let (bytes, observation) = support::read_file(path)?;
                self.authenticate(&bytes, expected, locator, attempt, key)?;
                contents.push(bytes);
                present[index] = true;
                observations[index] = Some(observation);
            }
        }
        if contents.len() == 2 && contents[0] != contents[1] {
            return Err(support::changed());
        }
        self.verify()?;
        for (path, observation) in paths.iter().zip(&observations) {
            if let Some(observation) = observation {
                support::verify_file(path, observation)?;
            } else if support::present(path)? {
                return Err(support::changed());
            }
        }
        Ok(match present {
            [false, false] => AttemptState::Absent,
            [true, false] => AttemptState::AuthenticatedStaging,
            [false, true] => AttemptState::AuthenticatedPublishedCandidate,
            [true, true] => AttemptState::AuthenticatedStagingAndPublishedCandidate,
        })
    }

    fn authenticate(
        &self,
        bytes: &[u8],
        metadata: &ObjectMetadata,
        locator: &ObjectLocator,
        attempt: &AttemptId,
        key: &KeyEncryptionKey,
    ) -> Result<()> {
        let sealed = envelope::decode(bytes, metadata)?;
        drop(Zeroizing::new(open_object(key, metadata, &sealed)?));
        if AttemptId::for_sealed(locator, &sealed) != *attempt {
            return Err(SourceVaultError::new(
                SourceVaultErrorCode::AttemptMismatch,
                "authenticated object belongs to a different attempt",
            ));
        }
        Ok(())
    }
    fn verify(&self) -> Result<()> {
        self.root.verify()?;
        self.objects.verify()?;
        self.staging.verify()
    }
    fn object_path(&self, locator: &ObjectLocator) -> std::path::PathBuf {
        self.objects.path.join(format!("{}.rmo", locator.token()))
    }
    fn staging_path(&self, locator: &ObjectLocator, attempt: &AttemptId) -> std::path::PathBuf {
        self.staging
            .path
            .join(format!("{}.{}.stage", locator.token(), attempt.token()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Create,
    Write,
    Flush,
    FileSync,
    StagingSync,
    StagingReadBack,
    Publish,
    ObjectDirectorySync,
    PublishedReadBack,
    RemoveStaging,
    CleanupSync,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn validate_token(token: &str) -> Result<()> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::InvalidLocator,
            "invalid private object or attempt token",
        ));
    }
    Ok(())
}

macro_rules! redacted_debug {
    ($($kind:ty),+) => { $(impl fmt::Debug for $kind {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(concat!(stringify!($kind), "([REDACTED])")) }
    })+ };
}
redacted_debug!(
    ObjectLocator,
    AttemptId,
    ObjectWrite,
    PublishedObject,
    ObjectDirectory
);

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
