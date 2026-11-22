//! Where the server the bootstrap installs comes from (D § 4): a release
//! archive for the host's asset, a `remote.dev_server` binary, or, for a
//! development build without one, nowhere.

use crate::bootstrap::Asset;
use sha2::{Digest, Sha256};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A release archive, `tenmoku-server-<version>-<asset>.tar.gz`, already
/// verified: downloading and checking it is the caller's.
#[derive(Clone, Debug)]
pub enum Archive {
    Path(PathBuf),
    Bytes(Arc<[u8]>),
}

impl Archive {
    fn read(&self) -> io::Result<Arc<[u8]>> {
        match self {
            Archive::Path(path) => Ok(std::fs::read(path)?.into()),
            Archive::Bytes(bytes) => Ok(bytes.clone()),
        }
    }
}

/// The release archive for an asset.
pub type Release = Arc<dyn Fn(Asset) -> io::Result<Archive> + Send + Sync>;

#[derive(Clone)]
pub enum ServerSource {
    /// A release build: its archives, installed to `<build>/`.
    Release(Release),
    /// `remote.dev_server`: a local server binary for the host's target,
    /// uploaded with its own SHA-256 standing in for the signed sums and
    /// installed to `dev-<sha256[..12]>/`.
    Dev(PathBuf),
    /// A development build with no `remote.dev_server`. Nothing can be
    /// installed, and an install already on the host isn't used either:
    /// another checkout's changes can have the same build id.
    NoRelease,
}

impl ServerSource {
    /// The source for a client of `build`: `dev_server` if it's set (read
    /// from user settings only), else no release for a `-dirty` build,
    /// else `release`.
    pub fn for_build(build: &str, dev_server: Option<PathBuf>, release: Release) -> ServerSource {
        match dev_server {
            Some(path) => ServerSource::Dev(path),
            None if is_dev_build(build) => ServerSource::NoRelease,
            None => ServerSource::Release(release),
        }
    }
}

impl fmt::Debug for ServerSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerSource::Release(_) => f.write_str("Release(..)"),
            ServerSource::Dev(path) => f.debug_tuple("Dev").field(path).finish(),
            ServerSource::NoRelease => f.write_str("NoRelease"),
        }
    }
}

/// Whether `build` can have no release asset: one built from a tree with
/// changes. A clean but unreleased commit is found out at the download.
pub fn is_dev_build(build: &str) -> bool {
    build.ends_with("-dirty")
}

/// What an upload sends: a `.tar.gz` with `tenmoku-server` at its root,
/// and its SHA-256 in hex.
#[derive(Clone)]
pub struct Payload {
    pub bytes: Arc<[u8]>,
    pub sha256: String,
}

impl Payload {
    pub fn new(bytes: Arc<[u8]>) -> Payload {
        let sha256 = hex(&Sha256::digest(&bytes));
        Payload { bytes, sha256 }
    }

    /// A release archive, as it is.
    pub fn archive(archive: &Archive) -> io::Result<Payload> {
        archive.read().map(Payload::new)
    }
}

/// A `remote.dev_server` binary, read and packed for upload.
pub struct DevServer {
    /// `dev-<sha256[..12]>`, of the binary.
    pub dir: String,
    pub payload: Payload,
}

impl DevServer {
    pub fn read(path: &Path) -> io::Result<DevServer> {
        let binary = std::fs::read(path)?;
        let dir = format!("dev-{}", &hex(&Sha256::digest(&binary))[..12]);
        Ok(DevServer {
            dir,
            payload: Payload::new(pack(&binary)?.into()),
        })
    }
}

/// A `.tar.gz` holding `binary` as `tenmoku-server`, mode 755.
pub fn pack(binary: &[u8]) -> io::Result<Vec<u8>> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    let mut header = tar::Header::new_ustar();
    header.set_size(binary.len() as u64);
    header.set_mode(0o755);
    header.set_entry_type(tar::EntryType::Regular);
    tar.append_data(&mut header, "tenmoku-server", binary)?;
    tar.into_inner()?.finish()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
