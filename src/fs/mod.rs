use alloc::string::String;
use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::RwLock;
use core::fmt;

pub mod memfs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
}

#[derive(Debug)]
pub enum FsError {
    NotFound,
    AlreadyExists,
    NotADirectory,
    NotAFile,
    InvalidPath,
    PermissionDenied,
}

pub type Result<T> = core::result::Result<T, FsError>;

#[derive(Debug, Clone)]
pub struct FileStats {
    pub file_type: FileType,
    pub size: usize,
    pub permissions: u16,
}

pub trait Filesystem: Send + Sync {
    fn root_dir(&self) -> Arc<dyn Directory>;
    fn create_file(&self, path: &str, data: Vec<u8>) -> Result<()>;
    fn create_dir(&self, path: &str) -> Result<()>;
    fn remove(&self, path: &str) -> Result<()>;
    fn get_file(&self, path: &str) -> Result<Arc<dyn File>>;
    fn get_dir(&self, path: &str) -> Result<Arc<dyn Directory>>;
}

pub trait File: Send + Sync {
    fn read(&self) -> Result<Vec<u8>>;
    fn write(&self, data: &[u8]) -> Result<()>;
    fn append(&self, data: &[u8]) -> Result<()>;
    fn truncate(&self) -> Result<()>;
    fn stats(&self) -> Result<FileStats>;
}

pub trait Directory: Send + Sync {
    fn list(&self) -> Result<Vec<(String, FileType)>>;
    fn get_file(&self, name: &str) -> Result<Arc<dyn File>>;
    fn get_dir(&self, name: &str) -> Result<Arc<dyn Directory>>;
    fn create_file(&self, name: &str, data: Vec<u8>) -> Result<()>;
    fn create_dir(&self, name: &str) -> Result<()>;
    fn remove(&self, name: &str) -> Result<()>;
    fn stats(&self) -> Result<FileStats>;
}

lazy_static::lazy_static! {
    pub static ref ROOT_FS: Arc<RwLock<Arc<dyn Filesystem>>> = {
        Arc::new(RwLock::new(Arc::new(memfs::MemFs::new())))
    };
}

pub fn init() {
    // Initialize the root filesystem
    println!("Initializing filesystem...");
    let fs = ROOT_FS.read();
    let root = fs.root_dir();
    
    // Create some initial directories
    let _ = fs.create_dir("/bin");
    let _ = fs.create_dir("/home");
    let _ = fs.create_dir("/tmp");
    
    println!("Filesystem initialized successfully!");
} 