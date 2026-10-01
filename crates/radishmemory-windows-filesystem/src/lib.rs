//! Borrowed-handle file identity only. No path, mutation, ownership transfer, or fallback API.
#![cfg(windows)]
#![deny(unsafe_code)]

use std::fs::File;
use std::io;
use std::os::windows::io::AsRawHandle;

use windows_sys::Win32::Storage::FileSystem::{
    FILE_ID_128, FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx,
};

/// Meaningful only while a handle keeps the original file alive; IDs can be reused after deletion.
/// Intentionally has no Debug implementation: filesystem identifiers are not diagnostics.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FileIdentity {
    volume: u64,
    id: [u8; 16],
}

/// Query the complete native ID of this exact open handle. Unsupported handles/filesystems fail.
/// The caller must retain a handle for as long as comparisons depend on the original identity.
#[allow(unsafe_code)]
pub fn file_identity(file: &File) -> io::Result<FileIdentity> {
    let mut info = FILE_ID_INFO {
        VolumeSerialNumber: 0,
        FileId: FILE_ID_128 {
            Identifier: [0; 16],
        },
    };
    // SAFETY: `file` owns a valid handle borrowed for this synchronous call. `info` is an initialized,
    // aligned FILE_ID_INFO with exactly the size requested for FileIdInfo. Windows writes only this
    // buffer and retains neither pointer. We neither close nor transfer the borrowed handle. On
    // failure the output is discarded and GetLastError is read before another Windows API call.
    let success = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(FileIdentity {
        volume: info.VolumeSerialNumber,
        id: info.FileId.Identifier,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn open_handles_distinguish_replacements_and_recognize_hard_links() {
        let root = std::env::temp_dir().join(format!(
            "radishmemory-file-identity-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
        let _cleanup = Cleanup(root.clone());
        let path = root.join("synthetic-original");
        let alias = root.join("synthetic-alias");
        let first = File::create_new(&path).unwrap();
        let identity = file_identity(&first).unwrap();
        fs::hard_link(&path, &alias).unwrap();
        let linked = File::open(&alias).unwrap();
        assert!(file_identity(&linked).unwrap() == identity);
        fs::remove_file(&path).unwrap();
        fs::remove_file(&alias).unwrap();
        let replacement = File::create_new(&path).unwrap();
        // Both original handles still keep the deleted file alive, excluding ID reuse.
        assert!(file_identity(&first).unwrap() == identity);
        assert!(file_identity(&replacement).unwrap() != identity);
    }

    #[test]
    fn handles_without_file_identity_fail_without_fallback() {
        let device = File::open("NUL").unwrap();
        assert!(file_identity(&device).is_err());
    }
}
