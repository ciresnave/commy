//! Memory Mapping Implementation for Phase 2
//!
//! This module provides the core memory mapping functionality using memmap2
//! for high-performance shared memory communication between processes.

use crate::manager::ManagerError;
use memmap2::{MmapMut, MmapOptions};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

/// A memory-mapped file wrapper with thread-safe operations
#[derive(Debug)]
pub struct MappedFile {
    /// The file path
    pub path: PathBuf,
    /// The underlying file handle
    file: Arc<RwLock<File>>,
    /// The memory map
    mmap: Arc<RwLock<MmapMut>>,
    /// File size in bytes
    pub size: u64,
    /// Whether the file was created by this instance
    created_by_us: bool,
}

impl MappedFile {
    /// Create a new memory-mapped file
    pub fn create<P: AsRef<Path>>(path: P, size: u64) -> Result<Self, ManagerError> {
        let path = path.as_ref().to_path_buf();

        info!(
            "Creating memory-mapped file at {:?} with size {} bytes",
            path, size
        );

        // Create the file with the specified size
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .map_err(|e| {
                let com_err = crate::errors::CommyError::Io {
                    source: e,
                    path: None,
                };
                crate::manager::map_commy_error_to_manager_error(com_err, Some(path.clone()), None)
            })?;

        // Set the file size
        file.set_len(size).map_err(|e| {
            let com_err = crate::errors::CommyError::Io {
                source: e,
                path: Some(path.clone()),
            };
            crate::manager::map_commy_error_to_manager_error(com_err, Some(path.clone()), None)
        })?;

        // Create the memory map
        let mut mmap = unsafe {
            MmapOptions::new()
                .len(size as usize)
                .map_mut(&file)
                .map_err(|e| {
                    let com_err = crate::errors::CommyError::Io {
                        source: e,
                        path: Some(path.clone()),
                    };
                    crate::manager::map_commy_error_to_manager_error(
                        com_err,
                        Some(path.clone()),
                        None,
                    )
                })?
        };

        // Initialize the file with zeros
        for byte in &mut mmap[..] {
            *byte = 0;
        }

        // Flush to ensure the file is written
        mmap.flush().map_err(|e| {
            let com_err = crate::errors::CommyError::Io {
                source: e,
                path: Some(path.clone()),
            };
            crate::manager::map_commy_error_to_manager_error(com_err, Some(path.clone()), None)
        })?;

        info!("Successfully created memory-mapped file at {:?}", path);

        Ok(MappedFile {
            path,
            file: Arc::new(RwLock::new(file)),
            mmap: Arc::new(RwLock::new(mmap)),
            size,
            created_by_us: true,
        })
    }

    /// Open an existing memory-mapped file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, ManagerError> {
        let path = path.as_ref().to_path_buf();

        info!("Opening existing memory-mapped file at {:?}", path);

        if !path.exists() {
            return Err(ManagerError::FileNotFound {
                identifier: path.to_string_lossy().to_string(),
            });
        }

        // Open the existing file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| {
                let com_err = crate::errors::CommyError::Io {
                    source: e,
                    path: None,
                };
                crate::manager::map_commy_error_to_manager_error(com_err, Some(path.clone()), None)
            })?;

        // Get the file size
        let metadata = file.metadata().map_err(|e| {
            let com_err = crate::errors::CommyError::Io {
                source: e,
                path: Some(path.clone()),
            };
            crate::manager::map_commy_error_to_manager_error(com_err, Some(path.clone()), None)
        })?;
        let size = metadata.len();

        if size == 0 {
            return Err(ManagerError::IoError {
                path: path.clone(),
                message: "File is empty".to_string(),
            });
        }

        // Create the memory map
        let mmap = unsafe {
            MmapOptions::new()
                .len(size as usize)
                .map_mut(&file)
                .map_err(|e| {
                    let com_err = crate::errors::CommyError::Io {
                        source: e,
                        path: Some(path.clone()),
                    };
                    crate::manager::map_commy_error_to_manager_error(
                        com_err,
                        Some(path.clone()),
                        None,
                    )
                })?
        };

        info!(
            "Successfully opened memory-mapped file at {:?} with size {} bytes",
            path, size
        );

        Ok(MappedFile {
            path,
            file: Arc::new(RwLock::new(file)),
            mmap: Arc::new(RwLock::new(mmap)),
            size,
            created_by_us: false,
        })
    }

    /// Write data to the memory-mapped file at a specific offset
    pub fn write_at(&self, offset: u64, data: &[u8]) -> Result<usize, ManagerError> {
        if offset + data.len() as u64 > self.size {
            return Err(ManagerError::InvalidOperation {
                operation: format!("write at offset {}", offset),
                topology: crate::manager::Topology::OneToOne, // Default topology
            });
        }

        let mut mmap = self.mmap.write().unwrap();
        let start = offset as usize;
        let end = start + data.len();

        // Copy data to the memory map
        mmap[start..end].copy_from_slice(data);

        debug!(
            "Wrote {} bytes at offset {} to {:?}",
            data.len(),
            offset,
            self.path
        );
        Ok(data.len())
    }

    /// Read data from the memory-mapped file at a specific offset
    pub fn read_at(&self, offset: u64, length: usize) -> Result<Vec<u8>, ManagerError> {
        if offset + length as u64 > self.size {
            return Err(ManagerError::InvalidOperation {
                operation: format!("read at offset {}", offset),
                topology: crate::manager::Topology::OneToOne, // Default topology
            });
        }

        let mmap = self.mmap.read().unwrap();
        let start = offset as usize;
        let end = start + length;

        let data = mmap[start..end].to_vec();

        debug!(
            "Read {} bytes from offset {} from {:?}",
            length, offset, self.path
        );
        Ok(data)
    }

    /// Flush changes to disk
    pub fn flush(&self) -> Result<(), ManagerError> {
        let mmap = self.mmap.read().unwrap();
        mmap.flush().map_err(|e| {
            let com_err = crate::errors::CommyError::Io {
                source: e,
                path: Some(self.path.clone()),
            };
            crate::manager::map_commy_error_to_manager_error(com_err, Some(self.path.clone()), None)
        })?;

        let file = self.file.read().unwrap();
        file.sync_all().map_err(|e| {
            let com_err = crate::errors::CommyError::Io {
                source: e,
                path: Some(self.path.clone()),
            };
            crate::manager::map_commy_error_to_manager_error(com_err, Some(self.path.clone()), None)
        })?;

        debug!("Flushed changes to disk for {:?}", self.path);
        Ok(())
    }

    /// Get the entire file content as a slice
    pub fn as_slice(&self) -> Vec<u8> {
        let mmap = self.mmap.read().unwrap();
        mmap[..].to_vec()
    }

    /// Resize the memory-mapped file (only if we created it)
    pub fn resize(&mut self, new_size: u64) -> Result<(), ManagerError> {
        if !self.created_by_us {
            return Err(ManagerError::PermissionDenied {
                operation: "resize".to_string(),
                resource: "memory-mapped file".to_string(),
            });
        }

        if new_size == self.size {
            return Ok(());
        }

        info!(
            "Resizing memory-mapped file {:?} from {} to {} bytes",
            self.path, self.size, new_size
        );

        // Flush current changes
        self.flush()?;

        // Resize the underlying file
        {
            let file = self.file.write().unwrap();
            file.set_len(new_size).map_err(|e| {
                let com_err = crate::errors::CommyError::Io {
                    source: e,
                    path: None,
                };
                crate::manager::map_commy_error_to_manager_error(
                    com_err,
                    Some(self.path.clone()),
                    None,
                )
            })?;
            file.sync_all().map_err(|e| {
                let com_err = crate::errors::CommyError::Io {
                    source: e,
                    path: Some(self.path.clone()),
                };
                crate::manager::map_commy_error_to_manager_error(
                    com_err,
                    Some(self.path.clone()),
                    None,
                )
            })?;
        }

        // Recreate the memory map with new size
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(|e| {
                let com_err = crate::errors::CommyError::Io {
                    source: e,
                    path: None,
                };
                crate::manager::map_commy_error_to_manager_error(
                    com_err,
                    Some(self.path.clone()),
                    None,
                )
            })?;

        let mut new_mmap = unsafe {
            MmapOptions::new()
                .len(new_size as usize)
                .map_mut(&file)
                .map_err(|e| {
                    let com_err = crate::errors::CommyError::Io {
                        source: e,
                        path: None,
                    };
                    crate::manager::map_commy_error_to_manager_error(
                        com_err,
                        Some(self.path.clone()),
                        None,
                    )
                })?
        };

        // If growing, initialize new bytes to zero
        if new_size > self.size {
            let old_size = self.size as usize;
            let new_bytes = new_size as usize;
            for i in old_size..new_bytes {
                new_mmap[i] = 0;
            }
        }

        // Update our fields
        *self.file.write().unwrap() = file;
        *self.mmap.write().unwrap() = new_mmap;
        self.size = new_size;

        info!(
            "Successfully resized memory-mapped file {:?} to {} bytes",
            self.path, new_size
        );
        Ok(())
    }

    /// Get file statistics
    pub fn stats(&self) -> Result<FileStats, ManagerError> {
        let metadata = self.file.read().unwrap().metadata().map_err(|e| {
            let com_err = crate::errors::CommyError::Io {
                source: e,
                path: Some(self.path.clone()),
            };
            crate::manager::map_commy_error_to_manager_error(com_err, Some(self.path.clone()), None)
        })?;

        Ok(FileStats {
            size: metadata.len(),
            created: metadata.created().ok(),
            modified: metadata.modified().ok(),
            accessed: metadata.accessed().ok(),
        })
    }
}

impl Drop for MappedFile {
    fn drop(&mut self) {
        // Flush any remaining changes
        if let Err(e) = self.flush() {
            warn!(
                "Failed to flush memory-mapped file {:?} on drop: {}",
                self.path, e
            );
        }

        debug!("Dropping memory-mapped file {:?}", self.path);
    }
}

/// File statistics
#[derive(Debug, Clone)]
pub struct FileStats {
    pub size: u64,
    pub created: Option<std::time::SystemTime>,
    pub modified: Option<std::time::SystemTime>,
    pub accessed: Option<std::time::SystemTime>,
}

/// Memory mapping manager for handling multiple files
#[derive(Debug)]
pub struct MemoryMapManager {
    /// Base directory for memory-mapped files
    base_dir: PathBuf,
}

impl MemoryMapManager {
    /// Create a new memory mapping manager
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self, ManagerError> {
        let base_dir = base_dir.as_ref().to_path_buf();

        // Create base directory if it doesn't exist
        if !base_dir.exists() {
            std::fs::create_dir_all(&base_dir).map_err(|e| {
                let com_err = crate::errors::CommyError::Io {
                    source: e,
                    path: Some(base_dir.clone()),
                };
                crate::manager::map_commy_error_to_manager_error(
                    com_err,
                    Some(base_dir.clone()),
                    None,
                )
            })?;
            info!("Created memory map base directory: {:?}", base_dir);
        }

        Ok(MemoryMapManager { base_dir })
    }

    /// Create a new memory-mapped file with a unique name
    pub fn create_file(&self, file_id: u64, size: u64) -> Result<MappedFile, ManagerError> {
        let filename = format!("commy_file_{}.mmap", file_id);
        let path = self.base_dir.join(filename);

        MappedFile::create(path, size)
    }

    /// Create a new memory-mapped file using a provided filename (relative to base_dir)
    pub fn create_file_with_name<P: AsRef<Path>>(
        &self,
        _file_id: u64,
        filename: P,
        size: u64,
    ) -> Result<MappedFile, ManagerError> {
        let path = self.base_dir.join(filename);

        MappedFile::create(path, size)
    }

    /// Open an existing memory-mapped file
    pub fn open_file(&self, file_id: u64) -> Result<MappedFile, ManagerError> {
        let filename = format!("commy_file_{}.mmap", file_id);
        let path = self.base_dir.join(filename);

        MappedFile::open(path)
    }

    /// Check if a file exists
    pub fn file_exists(&self, file_id: u64) -> bool {
        let filename = format!("commy_file_{}.mmap", file_id);
        let path = self.base_dir.join(filename);
        path.exists()
    }

    /// Delete a memory-mapped file
    pub fn delete_file(&self, file_id: u64) -> Result<(), ManagerError> {
        let filename = format!("commy_file_{}.mmap", file_id);
        let path = self.base_dir.join(filename);

        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                let com_err = crate::errors::CommyError::Io {
                    source: e,
                    path: Some(path.clone()),
                };
                crate::manager::map_commy_error_to_manager_error(com_err, Some(path.clone()), None)
            })?;
            info!("Deleted memory-mapped file: {:?}", path);
        }

        Ok(())
    }

    /// List all memory-mapped files
    pub fn list_files(&self) -> Result<Vec<u64>, ManagerError> {
        let mut file_ids = Vec::new();

        for entry in std::fs::read_dir(&self.base_dir).map_err(|e| {
            let com_err = crate::errors::CommyError::Io {
                source: e,
                path: Some(self.base_dir.clone()),
            };
            crate::manager::map_commy_error_to_manager_error(
                com_err,
                Some(self.base_dir.clone()),
                None,
            )
        })? {
            let entry = entry.map_err(|e| {
                let com_err = crate::errors::CommyError::Io {
                    source: e,
                    path: Some(self.base_dir.clone()),
                };
                crate::manager::map_commy_error_to_manager_error(
                    com_err,
                    Some(self.base_dir.clone()),
                    None,
                )
            })?;
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            // Parse commy_file_{id}.mmap pattern
            if filename_str.starts_with("commy_file_") && filename_str.ends_with(".mmap") {
                let id_part = &filename_str[11..filename_str.len() - 5]; // Remove prefix and suffix
                if let Ok(file_id) = id_part.parse::<u64>() {
                    file_ids.push(file_id);
                }
            }
        }

        file_ids.sort();
        Ok(file_ids)
    }

    /// Clean up orphaned files (files that are no longer tracked)
    pub fn cleanup_orphaned_files(&self, active_file_ids: &[u64]) -> Result<usize, ManagerError> {
        let all_files = self.list_files()?;
        let mut cleaned = 0;

        for file_id in all_files {
            if !active_file_ids.contains(&file_id) {
                self.delete_file(file_id)?;
                cleaned += 1;
            }
        }

        if cleaned > 0 {
            info!("Cleaned up {} orphaned memory-mapped files", cleaned);
        }

        Ok(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_write_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.mmap");

        let mapped_file = MappedFile::create(&file_path, 1024).unwrap();

        // Write some data
        let data = b"Hello, Memory Mapped World!";
        let written = mapped_file.write_at(0, data).unwrap();
        assert_eq!(written, data.len());

        // Read it back
        let read_data = mapped_file.read_at(0, data.len()).unwrap();
        assert_eq!(read_data, data);

        // Test flush
        mapped_file.flush().unwrap();
    }

    #[test]
    fn test_open_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.mmap");

        // Create and write to file
        {
            let mapped_file = MappedFile::create(&file_path, 1024).unwrap();
            let data = b"Persistent data";
            mapped_file.write_at(100, data).unwrap();
            mapped_file.flush().unwrap();
        }

        // Open existing file
        let mapped_file = MappedFile::open(&file_path).unwrap();
        let read_data = mapped_file.read_at(100, 15).unwrap();
        assert_eq!(read_data, b"Persistent data");
    }

    #[test]
    fn test_memory_map_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryMapManager::new(temp_dir.path()).unwrap();

        // Create a file
        let mapped_file = manager.create_file(123, 2048).unwrap();
        assert_eq!(mapped_file.size, 2048);

        // Check it exists
        assert!(manager.file_exists(123));

        // List files
        let files = manager.list_files().unwrap();
        assert_eq!(files, vec![123]);

        // Clean up
        drop(mapped_file);
        manager.delete_file(123).unwrap();
        assert!(!manager.file_exists(123));
    }
}
