use russh_sftp::protocol::{
    Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode,
    Version,
};
use std::collections::HashMap;
use std::os::unix::prelude::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    {fs, io},
};
use tracing::{debug, error, info, warn};

/// Maintains the session state for an SFTP connection
pub struct SftpSession {
    /// Protocol version negotiated with a client
    version: Option<u32>,
    /// Root directory for this SFTP session
    root_dir: String,
    /// Map of open file/directory handles
    open_handles: HashMap<String, OpenHandle>,
    /// Counter for generating unique handle IDs
    next_handle_id: u64,
}

/// Holds file/directory information for open handles
pub struct OpenHandle {
    /// Whether this handle refers to a directory
    pub is_dir: bool,
    /// List of directory contents if this is a directory handle
    pub dir_contents: Option<Vec<String>>,
    /// Current index when reading directory contents
    pub dir_index: usize,
    /// Full path of the opened file/directory
    pub path: PathBuf,
    /// File handle (if this is a file)
    pub file: Option<fs::File>,
}

impl SftpSession {
    /// Creates a new SFTP session with the specified root directory
    pub fn new(root_dir: String) -> Self {
        debug!("Creating new SFTP session with root: {}", root_dir);
        Self {
            version: None,
            root_dir,
            open_handles: HashMap::new(),
            next_handle_id: 1,
        }
    }

    /// Generates a unique handle ID string
    fn generate_handle(&mut self) -> String {
        let handle_id = self.next_handle_id;
        self.next_handle_id += 1;
        format!("handle_{}", handle_id)
    }

    /// Creates a File object from a path with proper attributes
    async fn path_to_file(&self, path: &Path) -> io::Result<File> {
        let metadata = fs::metadata(path).await?;
        let attrs = FileAttributes {
            size: if metadata.is_file() { Some(metadata.len()) } else { None },
            uid: Some(metadata.uid()),
            gid: Some(metadata.gid()),
            permissions: Some(metadata.permissions().mode()),
            atime: metadata.accessed().ok().and_then(|t| {
                t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as u32)
            }),
            mtime: metadata.modified().ok().and_then(|t| {
                t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as u32)
            }),
            ..Default::default()
        };

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());

        Ok(File::new(file_name, attrs))
    }

    /// Normalizes and secures file paths within the root
    /// Prevents directory traversal attacks
    async fn normalize_path(&self, path: &str) -> io::Result<PathBuf> {
        debug!("Normalizing path: {}", path);
        let root_path = Path::new(&self.root_dir);

        // Handle empty or root path cases
        if path.is_empty() || path == "/" {
            return match root_path.canonicalize() {
                Ok(p) => Ok(p),
                Err(e) => {
                    error!("Root directory is invalid: {}", e);
                    Err(io::Error::new(io::ErrorKind::NotFound, e))
                }
            };
        }

        // Trim leading slash if present
        let trimmed_path = path.trim_start_matches('/');
        let target_path = root_path.join(trimmed_path);

        debug!("Target path after joining: {}", target_path.display());

        // Special handling for paths that don't exist yet
        if !target_path.exists() {
            return self.handle_nonexistent_path(target_path, root_path).await;
        }

        // For existing paths, canonicalize and check
        self.canonicalize_and_validate(target_path, root_path).await
    }

    /// Handle normalization for paths that don't exist yet
    async fn handle_nonexistent_path(
        &self,
        target_path: PathBuf,
        root_path: &Path,
    ) -> io::Result<PathBuf> {
        // Look for the closest existing parent
        let mut current = target_path.clone();
        let mut parents_to_create = Vec::new();

        while !current.exists() {
            if let Some(file_name) = current.file_name() {
                parents_to_create.push(file_name.to_os_string());
            }

            match current.parent() {
                Some(parent) => current = parent.to_path_buf(),
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "No valid parent path found",
                    ));
                }
            }
        }

        // Canonicalize the existing parent
        let canonical_parent = current.canonicalize().map_err(|e| {
            error!("Failed to canonicalize parent path: {}", e);
            io::Error::other(e)
        })?;

        // Check that the parent is within the root directory
        let canonical_root = root_path.canonicalize().map_err(|e| {
            error!("Failed to canonicalize root path: {}", e);
            io::Error::other(e)
        })?;

        if !canonical_parent.starts_with(&canonical_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Path traversal not allowed",
            ));
        }

        // Rebuild the path, appending the missing components in reverse order
        let mut result_path = canonical_parent;
        for component in parents_to_create.into_iter().rev() {
            result_path = result_path.join(component);
        }

        debug!("Normalized non-existent path: {}", result_path.display());
        Ok(result_path)
    }

    /// Canonicalize a path and validate it's within root
    async fn canonicalize_and_validate(
        &self,
        target_path: PathBuf,
        root_path: &Path,
    ) -> io::Result<PathBuf> {
        let canonical_path = target_path.canonicalize().map_err(|e| {
            error!("Failed to canonicalize path: {}", e);
            e
        })?;

        let canonical_root = root_path.canonicalize().map_err(|e| {
            error!("Failed to canonicalize root path: {}", e);
            io::Error::other(e)
        })?;

        if !canonical_path.starts_with(&canonical_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Path traversal not allowed",
            ));
        }

        debug!("Normalized existing path: {}", canonical_path.display());
        Ok(canonical_path)
    }
}

impl russh_sftp::server::Handler for SftpSession {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        warn!("Unimplemented SFTP operation requested");
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        version: u32,
        extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        if self.version.is_some() {
            error!("Duplicate SFTP init packet received");
            return Err(StatusCode::ConnectionLost);
        }

        self.version = Some(version);
        info!("SFTP version: {}, extensions: {:?}", version, extensions);

        Ok(Version::new())
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        info!("Opening file: {}, flags: {:?}", filename, pflags);

        let creating_file = pflags.contains(OpenFlags::CREATE);

        let path = self.normalize_path(&filename).await.map_err(|e| {
            warn!("Failed to normalize path '{}': {}", filename, e);
            StatusCode::NoSuchFile
        })?;

        // Ensure parent directories exist when creating files
        if creating_file
            && let Some(parent) = path.parent()
            && !parent.exists()
        {
            info!("Creating parent directories for: {}", path.display());
            fs::create_dir_all(parent).await.map_err(|e| {
                error!("Failed to create parent directories: {}", e);
                StatusCode::PermissionDenied
            })?;
        }

        // Configure file opening options
        let mut open_options = fs::OpenOptions::new();
        if pflags.contains(OpenFlags::READ) {
            open_options.read(true);
        }
        if pflags.contains(OpenFlags::WRITE) {
            open_options.write(true);
        }
        if pflags.contains(OpenFlags::CREATE) {
            open_options.create(true);
        }
        if pflags.contains(OpenFlags::TRUNCATE) {
            open_options.truncate(true);
        }
        if pflags.contains(OpenFlags::APPEND) {
            open_options.append(true);
        }

        // Open the file
        let file = open_options.open(&path).await.map_err(|e| {
            error!("Failed to open file {}: {}", path.display(), e);
            StatusCode::Failure
        })?;

        // Create and store the handle
        let handle = self.generate_handle();
        debug!("Created handle {} for file: {}", handle, path.display());

        self.open_handles.insert(
            handle.clone(),
            OpenHandle {
                is_dir: false,
                dir_contents: None,
                dir_index: 0,
                file: Some(file),
                path,
            },
        );

        Ok(Handle { id, handle })
    }

    async fn close(
        &mut self,
        id: u32,
        handle: String,
    ) -> Result<Status, Self::Error> {
        info!("Closing handle: {}", handle);
        if self.open_handles.remove(&handle).is_some() {
            debug!("Successfully closed handle: {}", handle);
        } else {
            warn!("Attempted to close non-existent handle: {}", handle);
        }
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        })
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        debug!(
            "Reading from handle: {}, offset: {}, length: {}",
            handle, offset, len
        );

        let open_handle =
            self.open_handles.get(&handle).ok_or(StatusCode::Failure)?;

        if open_handle.is_dir {
            warn!("Attempt to read from directory handle: {}", handle);
            return Err(StatusCode::Failure);
        }

        let mut file = fs::File::open(&open_handle.path)
            .await
            .map_err(|_| StatusCode::Failure)?;

        file.seek(io::SeekFrom::Start(offset))
            .await
            .map_err(|_| StatusCode::Failure)?;

        let mut buffer = vec![0u8; len as usize];
        let n =
            file.read(&mut buffer).await.map_err(|_| StatusCode::Failure)?;

        buffer.truncate(n);
        Ok(Data { id, data: buffer })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        debug!(
            "Writing to handle: {}, offset: {}, data_len: {}",
            handle,
            offset,
            data.len()
        );

        let open_handle =
            self.open_handles.get_mut(&handle).ok_or_else(|| {
                warn!("Invalid handle: {}", handle);
                StatusCode::Failure
            })?;

        if open_handle.is_dir {
            warn!("Attempt to write to directory handle: {}", handle);
            return Err(StatusCode::Failure);
        }

        let file = open_handle.file.as_mut().ok_or_else(|| {
            warn!("File handle is missing for: {}", handle);
            StatusCode::Failure
        })?;

        file.seek(io::SeekFrom::Start(offset)).await.map_err(|e| {
            error!("Failed to seek to offset {}: {}", offset, e);
            StatusCode::Failure
        })?;

        file.write_all(&data).await.map_err(|e| {
            error!("Failed to write data: {}", e);
            StatusCode::Failure
        })?;

        file.flush().await.map_err(|e| {
            error!("Failed to flush data: {}", e);
            StatusCode::Failure
        })?;

        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Write successful".to_string(),
            language_tag: "en-US".to_string(),
        })
    }

    async fn opendir(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Handle, Self::Error> {
        info!("Opening directory: {}", path);

        let full_path = self.normalize_path(&path).await.map_err(|e| {
            warn!("Failed to normalize path '{}': {}", path, e);
            StatusCode::NoSuchFile
        })?;

        let metadata = fs::metadata(&full_path).await.map_err(|e| {
            warn!(
                "Failed to read metadata for '{}': {}",
                full_path.display(),
                e
            );
            StatusCode::NoSuchFile
        })?;

        if !metadata.is_dir() {
            warn!("Path is not a directory: {}", full_path.display());
            return Err(StatusCode::NoSuchFile);
        }

        let mut entries = fs::read_dir(&full_path).await.map_err(|e| {
            warn!(
                "Permission denied reading directory '{}': {}",
                full_path.display(),
                e
            );
            StatusCode::PermissionDenied
        })?;

        let mut names = vec![];
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            warn!("Failed to read directory entry: {}", e);
            StatusCode::Failure
        })? {
            if let Ok(name) = entry.file_name().into_string() {
                names.push(name);
            }
        }

        let handle = self.generate_handle();
        debug!(
            "Created directory handle '{}' with {} entries",
            handle,
            names.len()
        );

        self.open_handles.insert(
            handle.clone(),
            OpenHandle {
                is_dir: true,
                dir_contents: Some(names),
                dir_index: 0,
                path: full_path,
                file: None,
            },
        );

        Ok(Handle { id, handle })
    }

    async fn readdir(
        &mut self,
        id: u32,
        handle: String,
    ) -> Result<Name, Self::Error> {
        debug!("Reading directory handle: {}", handle);

        let (
            dir_contents,
            current_dir_path,
            start_idx,
            end_idx,
            is_first_batch,
        ) = {
            let open_handle =
                self.open_handles.get_mut(&handle).ok_or_else(|| {
                    warn!("Invalid directory handle: {}", handle);
                    StatusCode::Failure
                })?;

            if !open_handle.is_dir {
                warn!("Handle {} is not a directory", handle);
                return Err(StatusCode::Failure);
            }

            // Check if we've reached EOF
            let contents = open_handle.dir_contents.as_ref().unwrap();
            if open_handle.dir_index >= contents.len() {
                debug!("End of directory listing for {}", handle);
                return Err(StatusCode::Eof);
            }

            // Get a batch of entries (up to 100 at a time)
            let batch_size = 100;
            let start_idx = open_handle.dir_index;
            let end_idx = std::cmp::min(start_idx + batch_size, contents.len());

            let file_names: Vec<String> = contents[start_idx..end_idx].to_vec();
            let path = open_handle.path.clone();
            let is_first_batch = start_idx == 0;

            // Update the index for the next read
            open_handle.dir_index = end_idx;

            (file_names, path, start_idx, end_idx, is_first_batch)
        };

        let mut files = Vec::new();

        // Only add (. & ..) on the first batch
        if is_first_batch {
            files.push(File::new(
                ".".to_string(),
                FileAttributes {
                    permissions: Some(0o755),
                    ..Default::default()
                },
            ));
            files.push(File::new(
                "..".to_string(),
                FileAttributes {
                    permissions: Some(0o755),
                    ..Default::default()
                },
            ));
        }

        // Process each file in the batch
        for filename in dir_contents {
            let path_buf = current_dir_path.join(&filename);
            match self.path_to_file(&path_buf).await {
                Ok(file) => {
                    files.push(file);
                }
                Err(e) => {
                    warn!("Failed to get file info for {}: {}", filename, e);
                    let file = File::new(filename, FileAttributes::default());
                    files.push(file);
                }
            }
        }

        debug!(
            "Returning {} files for directory listing (batch {}-{})",
            files.len(),
            start_idx,
            end_idx
        );
        Ok(Name { id, files })
    }

    async fn remove(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Status, Self::Error> {
        info!("Remove file: {}", path);

        let full_path = self
            .normalize_path(&path)
            .await
            .map_err(|_| StatusCode::NoSuchFile)?;

        if !full_path.exists() {
            warn!("Path does not exist: {}", full_path.display());
            return Err(StatusCode::NoSuchFile);
        }

        let metadata = fs::metadata(&full_path).await.map_err(|e| {
            error!("Failed to get metadata for {}: {}", full_path.display(), e);
            StatusCode::NoSuchFile
        })?;

        if !metadata.is_file() {
            warn!("{} is not a file", full_path.display());
            return Err(StatusCode::Failure);
        }

        fs::remove_file(&full_path).await.map_err(|e| {
            error!("Failed to remove file {}: {}", full_path.display(), e);
            StatusCode::Failure
        })?;

        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        })
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        info!("Create directory: {}", path);

        let full_path = self.normalize_path(&path).await.map_err(|e| {
            warn!("Failed to normalize path '{}': {}", path, e);
            StatusCode::NoSuchFile
        })?;

        if full_path.exists() {
            if full_path.is_dir() {
                debug!("Directory already exists: {}", full_path.display());
                return Ok(Status {
                    id,
                    status_code: StatusCode::Ok,
                    error_message: "Directory already exists".to_string(),
                    language_tag: "en-US".to_string(),
                });
            } else {
                warn!(
                    "Path exists but is not a directory: {}",
                    full_path.display()
                );
                return Err(StatusCode::Failure);
            }
        }

        fs::create_dir_all(&full_path).await.map_err(|e| {
            error!("Failed to create directory {}: {}", full_path.display(), e);
            StatusCode::Failure
        })?;

        info!("Successfully created directory: {}", full_path.display());
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        })
    }

    async fn rmdir(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Status, Self::Error> {
        info!("Remove directory: {}", path);

        let full_path = self
            .normalize_path(&path)
            .await
            .map_err(|_| StatusCode::NoSuchFile)?;

        if !full_path.exists() {
            warn!("Path does not exist: {}", full_path.display());
            return Err(StatusCode::NoSuchFile);
        }

        let metadata = fs::metadata(&full_path).await.map_err(|e| {
            error!("Failed to get metadata for {}: {}", full_path.display(), e);
            StatusCode::NoSuchFile
        })?;

        if !metadata.is_dir() {
            warn!("{} is not a directory", full_path.display());
            return Err(StatusCode::Failure);
        }

        fs::remove_dir(&full_path).await.map_err(|e| {
            error!("Failed to remove directory {}: {}", full_path.display(), e);
            StatusCode::Failure
        })?;

        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        })
    }

    async fn realpath(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Name, Self::Error> {
        debug!("Realpath request for: {}", path);

        let norm = if path.is_empty() || path == "/" {
            "/".to_string()
        } else {
            format!("/{}", path.trim_start_matches('/'))
        };

        let file = File::dummy(&norm);
        debug!("Resolved realpath '{}' to '{}'", path, norm);
        Ok(Name { id, files: vec![file] })
    }

    async fn stat(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<russh_sftp::protocol::Attrs, Self::Error> {
        debug!("Stat request for: {}", path);

        let full_path = self.normalize_path(&path).await.map_err(|e| {
            warn!("Failed to normalize path '{}': {}", path, e);
            StatusCode::NoSuchFile
        })?;

        let metadata = fs::metadata(&full_path).await.map_err(|e| {
            warn!("Failed to stat file '{}': {}", full_path.display(), e);
            StatusCode::NoSuchFile
        })?;

        let attrs = FileAttributes {
            size: Some(metadata.len()),
            uid: Some(metadata.uid()),
            gid: Some(metadata.gid()),
            permissions: Some(metadata.permissions().mode()),
            atime: metadata.accessed().ok().and_then(|t| {
                t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as u32)
            }),
            mtime: metadata.modified().ok().and_then(|t| {
                t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as u32)
            }),
            ..Default::default()
        };

        debug!(
            "Stat successful for '{}': size={:?}, perms={:?}",
            path, attrs.size, attrs.permissions
        );
        Ok(russh_sftp::protocol::Attrs { id, attrs })
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        info!("Rename: {} to {}", oldpath, newpath);

        let old_full_path = self
            .normalize_path(&oldpath)
            .await
            .map_err(|_| StatusCode::NoSuchFile)?;

        let new_full_path = self
            .normalize_path(&newpath)
            .await
            .map_err(|_| StatusCode::NoSuchFile)?;

        if !old_full_path.exists() {
            warn!("Old path does not exist: {}", old_full_path.display());
            return Err(StatusCode::NoSuchFile);
        }

        fs::rename(&old_full_path, &new_full_path).await.map_err(|e| {
            error!(
                "Failed to rename {} to {}: {}",
                old_full_path.display(),
                new_full_path.display(),
                e
            );
            StatusCode::Failure
        })?;

        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        })
    }
}
