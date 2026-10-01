//! Precise removal after a persisted retirement decision. Never scans for garbage.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetirementStep {
    BeforeRemoval,
    StagingRemoved,
    ObjectRemoved,
    DirectoriesSynced,
}

impl ObjectDirectory {
    pub(crate) fn retire_attempt<E: From<SourceVaultError>>(
        &self,
        locator: &ObjectLocator,
        attempt: &AttemptId,
        metadata: &ObjectMetadata,
        key: &KeyEncryptionKey,
        mut step: impl FnMut(RetirementStep) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        self.inspect_attempt(locator, attempt, metadata, key)?;
        let paths = [
            self.staging_path(locator, attempt),
            self.object_path(locator),
        ];
        // Hold file references across both removals to detect replacement and ID reuse.
        let mut observations = [None, None];
        for (path, observed) in paths.iter().zip(observations.iter_mut()) {
            if support::present(path)? {
                let (bytes, identity) = support::read_file(path)?;
                self.authenticate(&bytes, metadata, locator, attempt, key)?;
                *observed = Some(identity);
            }
        }
        if observations.iter().all(Option::is_some) && observations[0] != observations[1] {
            return Err(support::changed().into());
        }
        let mut removed = [false; 2];
        step(RetirementStep::BeforeRemoval)?;
        for index in 0..2 {
            self.verify()?;
            // Also require previously absent/removed names to remain absent.
            for (position, path) in paths.iter().enumerate() {
                match (&observations[position], removed[position]) {
                    (Some(expected), false) => support::verify_file(path, expected)?,
                    _ if support::present(path)? => return Err(support::changed().into()),
                    _ => (),
                }
            }
            if observations[index].is_some() {
                fs::remove_file(&paths[index])
                    .map_err(|e| SourceVaultError::io("remove retired object entry", e))?;
                removed[index] = true;
            }
            step(if index == 0 {
                RetirementStep::StagingRemoved
            } else {
                RetirementStep::ObjectRemoved
            })?;
        }
        self.verify()?;
        self.staging.sync()?;
        self.objects.sync()?;
        step(RetirementStep::DirectoriesSynced)?;
        self.verify()?;
        for path in &paths {
            if support::present(path)? {
                return Err(support::changed().into());
            }
        }
        Ok(())
    }
}
