use alloc::string::{String, ToString};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::RwLock;
use crate::fs::{Directory, File, FileStats, FileType, FsError, Filesystem, Result};

pub struct MemFs {
    root: Arc<MemDir>,
}

impl MemFs {
    pub fn new() -> Self {
        Self {
            root: Arc::new(MemDir::new()),
        }
    }

    fn resolve_path<'a>(&self, path: &'a str) -> Result<(Arc<MemDir>, &'a str)> {
        if !path.starts_with('/') {
            return Err(FsError::InvalidPath);
        }

        let mut current_dir = Arc::clone(&self.root);
        let path = path.trim_start_matches('/');
        
        if path.is_empty() {
            return Ok((current_dir, ""));
        }

        let components: Vec<&str> = path.split('/').collect();
        let (file_name, parents) = components.split_last().unwrap();
        
        for component in parents {
            if component.is_empty() {
                continue;
            }
            current_dir = current_dir.get_dir_as_memdir(component)?;
        }

        Ok((current_dir, file_name))
    }
}

impl Filesystem for MemFs {
    fn root_dir(&self) -> Arc<dyn Directory> {
        Arc::clone(&self.root) as Arc<dyn Directory>
    }

    fn create_file(&self, path: &str, data: Vec<u8>) -> Result<()> {
        let (dir, name) = self.resolve_path(path)?;
        dir.create_file(name, data)
    }

    fn create_dir(&self, path: &str) -> Result<()> {
        let (dir, name) = self.resolve_path(path)?;
        dir.create_dir(name)
    }

    fn remove(&self, path: &str) -> Result<()> {
        let (dir, name) = self.resolve_path(path)?;
        dir.remove(name)
    }

    fn get_file(&self, path: &str) -> Result<Arc<dyn File>> {
        let (dir, name) = self.resolve_path(path)?;
        dir.get_file(name)
    }

    fn get_dir(&self, path: &str) -> Result<Arc<dyn Directory>> {
        let (dir, name) = self.resolve_path(path)?;
        dir.get_dir(name)
    }
}

pub struct MemFile {
    data: RwLock<Vec<u8>>,
}

impl MemFile {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data: RwLock::new(data),
        }
    }
}

impl File for MemFile {
    fn read(&self) -> Result<Vec<u8>> {
        Ok(self.data.read().clone())
    }

    fn write(&self, data: &[u8]) -> Result<()> {
        *self.data.write() = data.to_vec();
        Ok(())
    }

    fn append(&self, data: &[u8]) -> Result<()> {
        self.data.write().extend_from_slice(data);
        Ok(())
    }

    fn truncate(&self) -> Result<()> {
        self.data.write().clear();
        Ok(())
    }

    fn stats(&self) -> Result<FileStats> {
        Ok(FileStats {
            file_type: FileType::File,
            size: self.data.read().len(),
            permissions: 0o644,
        })
    }
}

pub struct MemDir {
    entries: RwLock<BTreeMap<String, Entry>>,
}

enum Entry {
    File(Arc<MemFile>),
    Directory(Arc<MemDir>),
}

impl MemDir {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(BTreeMap::new()),
        }
    }

    fn get_dir_as_memdir(&self, name: &str) -> Result<Arc<MemDir>> {
        let entries = self.entries.read();
        match entries.get(name) {
            Some(Entry::Directory(dir)) => Ok(Arc::clone(dir)),
            Some(_) => Err(FsError::NotADirectory),
            None => Err(FsError::NotFound),
        }
    }
}

impl Directory for MemDir {
    fn list(&self) -> Result<Vec<(String, FileType)>> {
        let entries = self.entries.read();
        let mut result = Vec::new();
        
        for (name, entry) in entries.iter() {
            let file_type = match entry {
                Entry::File(_) => FileType::File,
                Entry::Directory(_) => FileType::Directory,
            };
            result.push((name.clone(), file_type));
        }
        
        Ok(result)
    }

    fn get_file(&self, name: &str) -> Result<Arc<dyn File>> {
        let entries = self.entries.read();
        match entries.get(name) {
            Some(Entry::File(file)) => Ok(Arc::clone(file) as Arc<dyn File>),
            Some(_) => Err(FsError::NotAFile),
            None => Err(FsError::NotFound),
        }
    }

    fn get_dir(&self, name: &str) -> Result<Arc<dyn Directory>> {
        let dir = self.get_dir_as_memdir(name)?;
        Ok(dir as Arc<dyn Directory>)
    }

    fn create_file(&self, name: &str, data: Vec<u8>) -> Result<()> {
        let mut entries = self.entries.write();
        if entries.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }
        entries.insert(name.to_string(), Entry::File(Arc::new(MemFile::new(data))));
        Ok(())
    }

    fn create_dir(&self, name: &str) -> Result<()> {
        let mut entries = self.entries.write();
        if entries.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }
        entries.insert(name.to_string(), Entry::Directory(Arc::new(MemDir::new())));
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<()> {
        let mut entries = self.entries.write();
        if entries.remove(name).is_none() {
            return Err(FsError::NotFound);
        }
        Ok(())
    }

    fn stats(&self) -> Result<FileStats> {
        Ok(FileStats {
            file_type: FileType::Directory,
            size: self.entries.read().len(),
            permissions: 0o755,
        })
    }
} 