use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{LazyLock, Mutex};
use std::{env, fs};

use super::info::Info;
use super::registration;

pub(super) type Result<T> = std::result::Result<T, Error>;

const APPLICATIONS_DIR: &str = "/Applications";
const APPLICATION_SCRIPTS_DIR: &str = "Library/Application Scripts";
const CONTAINERS_DIR: &str = "Library/Containers";
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
pub(super) const PLUGINKIT: &str = "/usr/bin/pluginkit";
const CODESIGN: &str = "/usr/bin/codesign";
const XATTR: &str = "/usr/bin/xattr";
const FSKIT_EXTENSION_POINT: &str = "com.apple.fskit.fsmodule";
const FSKIT_APPEX_RELATIVE_PATH: &str = "Contents/Extensions/FSKitExt.appex";

static INSTALLER_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub(super) fn install(source: &Path) -> Result<()> {
    let _guard = INSTALLER_LOCK.lock().expect("installer mutex poisoned");

    if !source.is_dir() {
        return Err(Error::InvalidSource);
    }

    let Some(app_name) = source.file_name() else {
        return Err(Error::InvalidSource);
    };
    let app_path = app_path(app_name);

    if app_path.exists() {
        return Err(Error::AppInstalled);
    }

    let parent = app_path.parent().unwrap();
    fs::create_dir_all(parent)?;

    verify_app(source)?;

    // Copy the host app bundle into its final installation path.
    run_cmd(
        "ditto",
        &[source.to_str().unwrap(), app_path.to_str().unwrap()],
    )?;

    verify_app(&app_path)?;

    activate_impl(app_name)
}

pub(super) fn uninstall(app_name: &OsStr) -> Result<()> {
    let _guard = INSTALLER_LOCK.lock().expect("installer mutex poisoned");

    let app_path = app_path(app_name);

    if !app_path.exists() {
        return Err(Error::AppNotInstalled);
    }

    let appex = appex_path(&app_path)?;
    let appex_id = appex_bundle_id(&app_path)?;
    let app_id = app_bundle_id(&app_path)?;

    unregister_path(&app_path, &appex);
    unregister_registrations(&appex_id);
    remove_app_state(&app_id, &appex_id)?;

    fs::remove_dir_all(&app_path)?;

    Ok(())
}

pub(super) fn activate(app_name: &OsStr) -> Result<()> {
    let _guard = INSTALLER_LOCK.lock().expect("installer mutex poisoned");
    activate_impl(app_name)
}

fn activate_impl(app_name: &OsStr) -> Result<()> {
    let app_path = app_path(app_name);

    if !app_path.exists() {
        return Err(Error::AppNotInstalled);
    }

    verify_app(&app_path)?;

    let appex = appex_path(&app_path)?;
    let appex_id = appex_bundle_id(&app_path)?;

    if is_active(&appex_id, &app_path)? {
        return Ok(());
    }

    clear_quarantine(&app_path)?;

    register_app(&app_path)?;
    register_ext(&appex)?;
    elect_ext(&appex_id);

    if is_active(&appex_id, &app_path)? {
        return Ok(());
    }

    Err(Error::ExtensionNotActivated {
        appex_id,
        reason: failure_reason(&app_path)?,
    })
}

pub(super) fn app_path(app_name: &OsStr) -> PathBuf {
    Path::new(APPLICATIONS_DIR).join(app_name)
}

pub(super) fn appex_path(app_path: &Path) -> Result<PathBuf> {
    let appex = app_path.join(FSKIT_APPEX_RELATIVE_PATH);
    if !appex.exists() {
        return Err(Error::ExtensionNotFound);
    }
    Ok(appex)
}

fn appex_bundle_id(app_path: &Path) -> Result<String> {
    Ok(Info::new(&appex_path(app_path)?)?.bundle_id()?)
}

fn app_bundle_id(app_path: &Path) -> Result<String> {
    Ok(Info::new(app_path)?.bundle_id()?)
}

fn register_app(app_path: &Path) -> Result<()> {
    // Register the host app with LaunchServices.
    run_cmd(LSREGISTER, &["-f", "-R", app_path.to_str().unwrap()])
}

fn register_ext(appex_path: &Path) -> Result<()> {
    // Register the embedded FSKit extension with PlugInKit.
    run_cmd(PLUGINKIT, &["-a", appex_path.to_str().unwrap()])
}

fn elect_ext(appex_id: &str) {
    // Prefer this FSKit module when the system supports extension election.
    let _ = run_cmd(
        PLUGINKIT,
        &["-e", "use", "-p", FSKIT_EXTENSION_POINT, "-i", appex_id],
    );
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

fn verify_app(app: &Path) -> Result<()> {
    // Verify the app bundle signature and nested code signatures.
    run_cmd(
        CODESIGN,
        &[
            "--verify",
            "--deep",
            "--strict",
            "--verbose=2",
            app.to_str().unwrap(),
        ],
    )
}

fn failure_reason(app_path: &Path) -> Result<String> {
    let appex_id = appex_bundle_id(app_path)?;
    let statuses = registration::registrations(&appex_id)?;
    let matching: Vec<_> = statuses
        .iter()
        .filter(|status| status.app_path == app_path)
        .collect();

    if statuses.is_empty() {
        return Ok("registration not found after activation commands".to_string());
    }

    if matching.is_empty() {
        let other_paths = statuses
            .iter()
            .map(|status| status.app_path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Ok(format!(
            "extension is registered only for other app paths: {other_paths}"
        ));
    }

    if statuses.len() > 1 && !matching.iter().any(|status| status.elected) {
        return Ok("matching app registration exists but is not elected".to_string());
    }

    Ok(
        "matching app registration exists but the extension is still not considered active"
            .to_string(),
    )
}

fn unregister_registrations(appex_id: &str) {
    let Ok(statuses) = registration::registrations(appex_id) else {
        return;
    };

    for status in statuses {
        let appex = status.app_path.join(FSKIT_APPEX_RELATIVE_PATH);
        unregister_path(&status.app_path, &appex);
    }
}

fn unregister_path(app_path: &Path, appex_path: &Path) {
    if appex_path.exists() {
        // Best-effort unregister the embedded FSKit extension from PlugInKit.
        let _ = run_cmd(PLUGINKIT, &["-r", appex_path.to_str().unwrap()]);
    }

    // Best-effort unregister the host app from LaunchServices.
    let _ = run_cmd(LSREGISTER, &["-u", app_path.to_str().unwrap()]);
}

fn remove_app_state(app_id: &str, appex_id: &str) -> Result<()> {
    let Some(home) = env::var_os("HOME") else {
        return Ok(());
    };

    for path in [
        Path::new(&home).join(APPLICATION_SCRIPTS_DIR).join(app_id),
        Path::new(&home)
            .join(APPLICATION_SCRIPTS_DIR)
            .join(appex_id),
        Path::new(&home).join(CONTAINERS_DIR).join(app_id),
        Path::new(&home).join(CONTAINERS_DIR).join(appex_id),
    ] {
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
    }

    Ok(())
}

fn is_active(appex_id: &str, app_path: &Path) -> Result<bool> {
    let statuses = registration::registrations(appex_id)?;
    let matching: Vec<_> = statuses
        .iter()
        .filter(|status| status.app_path == app_path)
        .collect();

    if statuses.len() > 1 {
        Ok(matching.iter().any(|status| status.elected))
    } else {
        Ok(!matching.is_empty())
    }
}

fn run_cmd(cmd: &'static str, args: &[&str]) -> Result<()> {
    run_cmd_out(cmd, args).map(|_| ())
}

pub(super) fn run_cmd_out(cmd: &'static str, args: &[&str]) -> Result<Output> {
    let output = Command::new(cmd).args(args).output()?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(Error::CommandFailed {
            command: format!("{cmd} {}", args.join(" ")),
            status: describe_failure(&output),
        })
    }
}

pub(super) fn describe_failure(output: &Output) -> String {
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
    Info(#[from] super::info::Error),

    #[error("invalid host application source path")]
    InvalidSource,

    #[error("host application is not installed")]
    AppNotInstalled,

    #[error("host application is already installed")]
    AppInstalled,

    #[error("FSKit extension bundle not found in host application")]
    ExtensionNotFound,

    #[error("invalid FSKit extension path")]
    InvalidExtensionPath,

    #[error("FSKit extension is not active: {appex_id} ({reason})")]
    ExtensionNotActivated { appex_id: String, reason: String },

    #[error("command `{command}` failed: {status}")]
    CommandFailed { command: String, status: String },
}
