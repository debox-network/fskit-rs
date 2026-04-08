use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use async_trait::async_trait;

pub use crate::pb::check_access::AccessMask;
pub use crate::pb::preallocate_space::PreallocateFlag;
pub use crate::pb::set_xattr::SetXattrPolicy;
pub use crate::pb::supported_capabilities::CaseFormat;
pub use crate::pb::synchronize::SyncFlags;
pub use crate::pb::{
    DirectoryEntries, Item, ItemAttributes, ItemType, OpenMode, PathConfOperations,
    ResourceIdentifier, StatFsResult, SupportedCapabilities, TaskOptions, VolumeBehavior,
    VolumeIdentifier, Xattrs, directory_entries,
};
pub use crate::registration::Status;
use crate::session::Session;

mod handler;
mod info;
pub mod installer;
pub mod mounter;
mod registration;
pub mod session;
pub mod socket;

mod pb {
    include!(concat!(env!("OUT_DIR"), "/pb.rs"));
}

const FSKIT_ID: &str = "network.debox.fskitbridge.fskitext";
const DEFAULT_MOUNT_POINT: &str = "/tmp/fskitbridge";

pub type Result<T> = std::result::Result<T, Error>;

#[async_trait]
pub trait Filesystem {
    /// Get the resource identifier and name.
    async fn get_resource_identifier(&mut self) -> Result<ResourceIdentifier>;

    /// Get the volume identifier and name.
    async fn get_volume_identifier(&mut self) -> Result<VolumeIdentifier>;

    /// Get options that tell FSKit to declare behaviors and selectively inhibit
    /// operation protocols.
    async fn get_volume_behavior(&mut self) -> Result<VolumeBehavior>;

    /// Get properties implemented by volumes that support providing the values of
    /// system limits or options.
    async fn get_path_conf_operations(&mut self) -> Result<PathConfOperations>;

    /// Get properties that provide the supported capabilities of the volume.
    async fn get_volume_capabilities(&mut self) -> Result<SupportedCapabilities>;

    /// Get properties that provide up-to-date statistics of the volume.
    async fn get_volume_statistics(&mut self) -> Result<StatFsResult>;

    /// Mounts this volume, using the specified options.
    async fn mount(&mut self, options: TaskOptions) -> Result<()>;

    /// Unmounts this volume.
    async fn unmount(&mut self) -> Result<()>;

    /// Synchronizes the volume with its underlying resource.
    async fn synchronize(&mut self, flags: SyncFlags) -> Result<()>;

    /// Fetches attributes for the given item.
    async fn get_attributes(&mut self, item_id: u64) -> Result<ItemAttributes>;

    /// Sets the given attributes on an item.
    async fn set_attributes(
        &mut self,
        item_id: u64,
        attributes: ItemAttributes,
    ) -> Result<ItemAttributes>;

    /// Looks up an item within a directory.
    async fn lookup_item(&mut self, name: &OsStr, directory_id: u64) -> Result<Item>;

    /// Reclaims an item, releasing any resources allocated for the item.
    async fn reclaim_item(&mut self, item_id: u64) -> Result<()>;

    /// Reads a symbolic link.
    async fn read_symbolic_link(&mut self, item_id: u64) -> Result<Vec<u8>>;

    /// Creates a new file or directory item.
    async fn create_item(
        &mut self,
        name: &OsStr,
        r#type: ItemType,
        directory_id: u64,
        attributes: ItemAttributes,
    ) -> Result<Item>;

    /// Creates a new symbolic link.
    async fn create_symbolic_link(
        &mut self,
        name: &OsStr,
        directory_id: u64,
        new_attributes: ItemAttributes,
        contents: Vec<u8>,
    ) -> Result<Item>;

    /// Creates a new hard link.
    async fn create_link(
        &mut self,
        item_id: u64,
        name: &OsStr,
        directory_id: u64,
    ) -> Result<Vec<u8>>;

    /// Removes an existing item from a given directory.
    async fn remove_item(&mut self, item_id: u64, name: &OsStr, directory_id: u64) -> Result<()>;

    /// Renames an item from one path in the file system to another.
    async fn rename_item(
        &mut self,
        item_id: u64,
        source_directory_id: u64,
        source_name: &OsStr,
        destination_name: &OsStr,
        destination_directory_id: u64,
        over_item_id: Option<u64>,
    ) -> Result<Vec<u8>>;

    /// Enumerates the contents of the given directory.
    async fn enumerate_directory(
        &mut self,
        directory_id: u64,
        cookie: u64,
        verifier: u64,
    ) -> Result<DirectoryEntries>;

    /// Activates the volume using the specified options.
    async fn activate(&mut self, options: TaskOptions) -> Result<Item>;

    /// Tears down a previously initialized volume instance.
    async fn deactivate(&mut self) -> Result<()>;

    /// Returns an array that specifies the extended attribute names the given
    /// item supports.
    async fn get_supported_xattr_names(&mut self, item_id: u64) -> Result<Xattrs>;

    /// Gets the specified extended attribute of the given item.
    async fn get_xattr(&mut self, name: &OsStr, item_id: u64) -> Result<Vec<u8>>;

    /// Sets the specified extended attribute data on the given item.
    async fn set_xattr(
        &mut self,
        name: &OsStr,
        value: Option<Vec<u8>>,
        item_id: u64,
        policy: SetXattrPolicy,
    ) -> Result<()>;

    /// Gets the list of extended attributes currently set on the given item.
    async fn get_xattrs(&mut self, item_id: u64) -> Result<Xattrs>;

    /// Opens a file for access.
    async fn open_item(&mut self, item_id: u64, modes: Vec<OpenMode>) -> Result<()>;

    /// Closes a file from further access.
    async fn close_item(&mut self, item_id: u64, modes: Vec<OpenMode>) -> Result<()>;

    /// Reads the contents of the given file item.
    async fn read(&mut self, item_id: u64, offset: i64, length: i64) -> Result<Vec<u8>>;

    /// Writes contents to the given file item.
    async fn write(&mut self, contents: Vec<u8>, item_id: u64, offset: i64) -> Result<i64>;

    /// Checks whether the file system allows access to the given item.
    async fn check_access(&mut self, item_id: u64, access: Vec<AccessMask>) -> Result<bool>;

    /// Sets a new name for the volume.
    async fn set_volume_name(&mut self, name: Vec<u8>) -> Result<Vec<u8>>;

    /// Preallocate disk space for the given item.
    async fn preallocate_space(
        &mut self,
        item_id: u64,
        offset: i64,
        length: i64,
        flags: Vec<PreallocateFlag>,
    ) -> Result<i64>;

    /// Notifies the file system that the kernel is no longer making immediate use of
    /// the given item.
    async fn deactivate_item(&mut self, item_id: u64) -> Result<()>;
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("POSIX error: {0}")]
    Posix(std::ffi::c_int),
}

/// Configuration for mounting the file system and connecting between `FSKitExt` and `fskit-rs`.
///
/// # Parameters
/// * `fskit_id` — Bundle identifier of the FSKit extension used for registration
///   and election. Default: `network.debox.fskitbridge.fskitext`.
/// * `mount_point` — Existing (usually empty) directory to mount onto. Use `/Volumes/<Name>`
///   (may require `sudo`) or a user-owned path. Default: `/tmp/fskitbridge`.
/// * `force` — If `true`, preflight **unmounts** anything already mounted at `mount_point`
///   before mounting. Default: `true`.
#[derive(Debug, Clone)]
pub struct MountOptions {
    pub fskit_id: String,
    pub mount_point: PathBuf,
    pub force: bool,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            fskit_id: FSKIT_ID.into(),
            mount_point: PathBuf::from(DEFAULT_MOUNT_POINT),
            force: true,
        }
    }
}

/// Mounts a user-space file system at `opts.mount_point` and returns a `Session` that
/// keeps the mount alive. Non-blocking: background workers serve kernel requests;
/// dropping `Session` cleanly unmounts.
///
/// # Parameters
/// * `fs` — Your `Filesystem` impl. Must be `Send + Sync + Clone + 'static`.
///   Prefer keeping a heavy state in `Arc<_>`.
/// * `opts` — Combined mount/connection configuration.
///
/// # Returns
/// A `Session` handle; while it’s alive, the mount remains active. Dropping it unmounts.
///
/// # macOS (FSKit) notes
/// * The extension must be **enabled** in System Settings (File System Extensions).
/// * FSKit mounts use `noowners`; you can store/report uid/gid in metadata,
///   but host POSIX enforcement will still be disabled.
pub async fn mount<FS>(fs: FS, opts: MountOptions) -> session::Result<Session>
where
    FS: Filesystem + Send + Sync + Clone + 'static,
{
    Session::new(fs, opts).await
}

/// Installs the FSKit host application into `destination` and registers its extension.
///
/// # Behavior
/// - If `force` is `true`, removes any existing app at `destination`.
/// - Copies the app bundle from `source` to `destination`.
/// - Registers the host app with LaunchServices.
/// - Registers the embedded `FSKitExt.appex` with PlugInKit.
/// - Clears `com.apple.quarantine` only when it is present on the installed app.
/// - Falls back to launching the installed host app only if command-line registration
///   was not enough.
///
/// # Commands
/// ```text
/// rm -rf <destination>
/// ditto <source> <destination>
/// xattr -dr com.apple.quarantine <destination>          # only if quarantine is present
/// lsregister -f -R <destination>
/// pluginkit -a <destination>/Contents/Extensions/FSKitExt.appex
/// pluginkit -e use -p com.apple.fskit.fsmodule -i <appex bundle id>
/// open <destination>                                    # fallback only
/// ```
pub fn install<P: AsRef<Path>, Q: AsRef<Path>>(
    source: P,
    destination: Q,
    force: bool,
) -> installer::Result<()> {
    installer::run(source.as_ref(), destination.as_ref(), force)
}

/// Uninstalls the FSKit host application from `destination`.
///
/// # Behavior
/// - Best-effort unregisters the embedded `FSKitExt.appex` from PlugInKit.
/// - Best-effort unregisters the host app from LaunchServices.
/// - Removes the app bundle from `destination`.
///
/// # Commands
/// ```text
/// pluginkit -r <destination>/Contents/Extensions/FSKitExt.appex   # best effort
/// lsregister -u <destination>                                     # best effort
/// rm -rf <destination>
/// ```
pub fn uninstall<P: AsRef<Path>>(destination: P) -> installer::Result<()> {
    installer::uninstall(destination.as_ref())
}

/// Returns all FSKit extension registrations matching `fskit_id`.
///
/// Each status item reports the extension path and whether PlugInKit marks it as elected.
pub fn registrations(fskit_id: &str) -> installer::Result<Vec<Status>> {
    registration::registrations(fskit_id)
}

#[macro_export]
macro_rules! path {
    ($arg:expr) => {
        $arg.as_os_str().to_str().unwrap()
    };
}
