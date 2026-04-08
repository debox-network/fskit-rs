use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use crate::info::Info;

pub(super) type Result<T> = std::result::Result<T, Error>;

const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
const PLUGINKIT: &str = "/usr/bin/pluginkit";
const XATTR: &str = "/usr/bin/xattr";
const OPEN: &str = "/usr/bin/open";
const FSKIT_EXTENSION_POINT: &str = "com.apple.fskit.fsmodule";
const FSKIT_APPEX_RELATIVE_PATH: &str = "Contents/Extensions/FSKitExt.appex";

pub(super) fn run(source: &Path, destination: &Path, force: bool) -> Result<()> {
    if !source.exists() {
        return Err(Error::AppNotFound);
    }

    let Some(parent) = destination.parent() else {
        return Err(Error::InvalidDestination);
    };

    let appex = destination.join(FSKIT_APPEX_RELATIVE_PATH);

    if destination.exists() {
        if !force {
            return Err(Error::AppInstalled);
        }
        fs::remove_dir_all(destination)?;
    }

    fs::create_dir_all(parent)?;

    // Copy the host app bundle into its final installation path.
    run_cmd(
        "ditto",
        &[source.to_str().unwrap(), destination.to_str().unwrap()],
    )?;

    if !appex.exists() {
        return Err(Error::ExtensionNotFound);
    }

    let bundle_id = Info::new(&appex)?.bundle_id()?;

    clear_quarantine(destination)?;

    // Register the host app with LaunchServices.
    run_cmd(LSREGISTER, &["-f", "-R", destination.to_str().unwrap()])?;
    // Register the embedded FSKit extension with PlugInKit.
    run_cmd(PLUGINKIT, &["-a", appex.to_str().unwrap()])?;

    // Prefer this FSKit module when the system supports extension election.
    let _ = run_cmd(
        PLUGINKIT,
        &[
            "-e",
            "use",
            "-p",
            FSKIT_EXTENSION_POINT,
            "-i",
            bundle_id.as_str(),
        ],
    );

    if is_registered(&bundle_id)? {
        return Ok(());
    }

    // Fall back to opening the host app once when CLI registration is not enough.
    run_cmd(OPEN, &[destination.to_str().unwrap()])?;

    if is_registered(&bundle_id)? {
        Ok(())
    } else {
        Err(Error::ExtensionNotRegistered { bundle_id })
    }
}

pub(super) fn uninstall(destination: &Path) -> Result<()> {
    if !destination.exists() {
        return Err(Error::AppNotInstalled);
    }

    let appex = destination.join(FSKIT_APPEX_RELATIVE_PATH);

    if appex.exists() {
        // Best-effort unregister the embedded FSKit extension first.
        let _ = run_cmd(PLUGINKIT, &["-r", appex.to_str().unwrap()]);
    }

    // Best-effort unregister the host app from LaunchServices.
    let _ = run_cmd(LSREGISTER, &["-u", destination.to_str().unwrap()]);

    fs::remove_dir_all(destination)?;

    Ok(())
}

fn clear_quarantine(app: &Path) -> Result<()> {
    let output = Command::new(XATTR)
        .args(["-p", "com.apple.quarantine", app.to_str().unwrap()])
        .output()?;

    if output.status.success() {
        // Clear quarantine only when the attribute is actually present.
        run_cmd(
            XATTR,
            &["-dr", "com.apple.quarantine", app.to_str().unwrap()],
        )?;
    }

    Ok(())
}

fn is_registered(bundle_id: &str) -> Result<bool> {
    let output = Command::new(PLUGINKIT)
        .args(["-m", "-i", bundle_id])
        .output()?;
    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains(bundle_id))
}

fn run_cmd(cmd: &'static str, args: &[&str]) -> Result<()> {
    let output = Command::new(cmd).args(args).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::CommandFailed {
            command: format!("{cmd} {}", args.join(" ")),
            status: describe_failure(&output),
        })
    }
}

fn describe_failure(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        output.status.to_string()
    } else {
        stderr
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Info(#[from] crate::info::Error),

    #[error("host application not found")]
    AppNotFound,

    #[error("invalid installation destination")]
    InvalidDestination,

    #[error("host application is already installed")]
    AppInstalled,

    #[error("host application is not installed")]
    AppNotInstalled,

    #[error("FSKit extension bundle not found in host application")]
    ExtensionNotFound,

    #[error("FSKit extension is not registered: {bundle_id}")]
    ExtensionNotRegistered { bundle_id: String },

    #[error("command `{command}` failed: {status}")]
    CommandFailed { command: String, status: String },
}
