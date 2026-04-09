use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use log::{error, info, warn};

use crate::installer::describe_failure;
use crate::{MountOptions, path};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub(super) struct Mounter {
    path: PathBuf,
    device: String,
}

impl Mounter {
    pub(super) fn mount(opts: MountOptions, fs_type: &str) -> Result<Self> {
        if !opts.mount_point.exists() {
            return Err(Error::MountPointMissing);
        }

        if opts.force
            && let Err(err) = unmount(&opts.mount_point)
        {
            warn!(
                "forced unmount of existing mount at {} failed: {err}",
                path!(opts.mount_point)
            );
        }

        let image = PathBuf::from(format!("/tmp/{fs_type}.dmg"));
        if !image.exists() {
            File::create(&image)?;
        }

        let device = attach_image(&image)?;

        let args = [
            "-F",
            "-t",
            fs_type,
            device.as_str(),
            path!(opts.mount_point),
        ];
        let mut process = Command::new("mount")
            .args(args)
            .stderr(Stdio::piped())
            .spawn()?;
        let start = Instant::now();
        loop {
            match process.try_wait()? {
                Some(status) => {
                    if status.success() {
                        break;
                    }
                    let stderr = process.wait_with_output()?.stderr;
                    let out = String::from_utf8_lossy(&stderr);
                    error!("{out}");
                    return if out.contains("is disabled") {
                        Err(Error::ExtensionDisabled)
                    } else if out.contains("Resource busy") {
                        Err(Error::MountPointBusy)
                    } else if out.contains("Probing resource") || out.contains("Loading resource") {
                        Err(Error::NeedReboot)
                    } else {
                        Err(Error::MountFailed)
                    };
                }
                None => {
                    if start.elapsed() >= Duration::from_secs(3) {
                        error!("mount command hung, killing process");
                        let _ = process.kill();
                        return Err(Error::NeedReboot);
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }

        info!(
            "file system mounted - type: {}, mount point: {} ({})",
            fs_type,
            path!(opts.mount_point),
            device
        );

        Ok(Self {
            path: opts.mount_point,
            device,
        })
    }

    pub(super) fn unmount(&self) -> Result<()> {
        unmount(&self.path)?;
        detach(&self.device)?;
        info!(
            "file system unmounted - mount point: {} ({})",
            path!(self.path),
            self.device
        );
        Ok(())
    }
}

fn attach_image(image: &Path) -> Result<String> {
    let args = [
        "attach",
        "-imagekey",
        "diskimage-class=CRawDiskImage",
        "-nomount",
        path!(image),
    ];
    let output = Command::new("hdiutil").args(args).output()?;
    if output.status.success() {
        let device = String::from_utf8(output.stdout)
            .map_err(|_| Error::InvalidDevice)?
            .trim()
            .to_string();
        if device.is_empty() {
            Err(Error::InvalidDevice)
        } else {
            Ok(device)
        }
    } else {
        Err(Error::AttachFailed(describe_failure(&output)))
    }
}

fn unmount(path: &PathBuf) -> Result<()> {
    let output = Command::new("umount").arg("-f").arg(path).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::UnmountFailed(describe_failure(&output)))
    }
}

fn detach(device: &str) -> Result<()> {
    let output = Command::new("hdiutil").args(["detach", device]).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::DetachFailed(describe_failure(&output)))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("mount point does not exist")]
    MountPointMissing,

    #[error("invalid device identifier returned by hdiutil")]
    InvalidDevice,

    #[error("failed to attach disk image: {0}")]
    AttachFailed(String),

    #[error("file system extension is disabled")]
    ExtensionDisabled,

    #[error("mount point is already in use")]
    MountPointBusy,

    #[error("file system extension was updated; reboot the system and try again")]
    NeedReboot,

    #[error("mount request failed")]
    MountFailed,

    #[error("unmount request failed: {0}")]
    UnmountFailed(String),

    #[error("failed to detach disk image: {0}")]
    DetachFailed(String),
}
