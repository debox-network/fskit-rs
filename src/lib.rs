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

/// Installs the FSKit host application into `/Applications/<source app name>`
/// and registers its extension.
///
/// # Behavior
/// - Derives the destination from the source app bundle name.
/// - Returns `AppInstalled` if `/Applications/<source app name>` already exists.
/// - Verifies the source app bundle signature before the copy step.
/// - Copies the app bundle from `source` to `/Applications/<source app name>`.
/// - Verifies the installed app bundle signature after the copy step.
/// - Activates the installed host app after the copy step.
///
/// # Observed macOS behavior
/// - This crate treats `/Applications/<source app name>` as the supported installation target.
/// - During local experiments, host apps from other locations may also work after the extension
///   is enabled, but that behavior is stateful and not guaranteed.
///
/// # Commands
/// ```text
/// codesign --verify --deep --strict --verbose=2 <source>
/// ditto <source> /Applications/<source app name>
/// codesign --verify --deep --strict --verbose=2 /Applications/<source app name>
/// activate(<source app name>)
/// ```
pub fn install<P: AsRef<Path>>(source: P) -> installer::Result<()> {
    installer::install(source.as_ref())
}

/// Activates an already installed FSKit host application from `/Applications/<app name>`.
///
/// # Behavior
/// - Verifies the installed app bundle signature before any registration step.
/// - Clears `com.apple.quarantine` only when it is present on the app.
/// - Registers the host app with LaunchServices.
/// - Registers the embedded `FSKitExt.appex` with PlugInKit.
/// - Requests election for the extension bundle id.
/// - Performs one activation check before registration and one after registration.
/// - Returns success only when the host app is considered active after the registration steps.
/// - On failure, reports whether the extension was never registered, only registered from a
///   different app path, or registered but not elected.
///
/// # Observed macOS behavior
/// - `activate()` always retries the registration steps when the host app is not already active.
/// - PlugInKit/ExtensionKit state may remain sensitive to prior registrations, app identities,
///   and install paths.
///
/// # Commands
/// ```text
/// codesign --verify --deep --strict --verbose=2 /Applications/<app name>
/// xattr -dr com.apple.quarantine /Applications/<app name>         # only if quarantine is present
/// lsregister -f -R /Applications/<app name>
/// pluginkit -a /Applications/<app name>/Contents/Extensions/FSKitExt.appex
/// pluginkit -e use -p com.apple.fskit.fsmodule -i <appex bundle id>
/// ```
pub fn activate<S: AsRef<OsStr>>(app_name: S) -> installer::Result<()> {
    installer::activate(app_name.as_ref())
}

/// Uninstalls the FSKit host application from `/Applications/<app name>`.
///
/// # Behavior
/// - Resolves the host app as `/Applications/<app name>`.
/// - Best-effort unregisters the embedded `FSKitExt.appex` from PlugInKit.
/// - Best-effort unregisters the host app from LaunchServices.
/// - Best-effort unregisters any remaining registrations for the same appex bundle id.
/// - Removes app-specific entries under `~/Library/Application Scripts` and `~/Library/Containers`.
/// - Removes the app bundle from `/Applications/<app name>`.
///
/// # Commands
/// ```text
/// pluginkit -r /Applications/<app name>/Contents/Extensions/FSKitExt.appex      # best effort
/// lsregister -u /Applications/<app name>                                        # best effort
/// pluginkit -m -i <appex bundle id> --raw                                       # inspect remaining registrations
/// pluginkit -r <other registered appex path>                                    # best effort
/// lsregister -u <other registered app path>                                     # best effort
/// rm -rf ~/Library/Application\ Scripts/<app bundle id>                         # best effort
/// rm -rf ~/Library/Application\ Scripts/<appex bundle id>                       # best effort
/// rm -rf ~/Library/Containers/<app bundle id>                                   # best effort
/// rm -rf ~/Library/Containers/<appex bundle id>                                 # best effort
/// rm -rf /Applications/<app name>
/// ```
pub fn uninstall<S: AsRef<OsStr>>(app_name: S) -> installer::Result<()> {
    installer::uninstall(app_name.as_ref())
}

#[macro_export]
macro_rules! path {
    ($arg:expr) => {
        $arg.as_os_str().to_str().unwrap()
    };
}
