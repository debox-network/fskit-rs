use std::path::{Path, PathBuf};

use log::error;
use regex::Regex;

use super::info::Info;
use super::installer::{Error, PLUGINKIT, Result, run_cmd_out};

#[derive(Debug, Clone)]
pub struct Status {
    pub appex_path: PathBuf,
    pub elected: bool,
}

pub(super) fn registrations(fskit_id: &str) -> Result<Vec<Status>> {
    let output = match run_cmd_out(PLUGINKIT, &["-m", "-i", fskit_id, "--raw"]) {
        Ok(output) => output,
        Err(err) => {
            error!("failed to query pluginkit for {fskit_id}: {err}");
            return Err(err);
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_statuses(&stdout))
}

pub(super) fn read_config(fskit_id: &str) -> Result<(u16, String)> {
    let statuses = registrations(fskit_id)?;

    let Some(status) = statuses
        .iter()
        .find(|status| status.elected)
        .or_else(|| statuses.first())
    else {
        error!("pluginkit did not return a registered path for {fskit_id}");
        return Err(Error::ExtensionNotRegistered {
            bundle_id: fskit_id.to_string(),
        });
    };

    let info = Info::new(Path::new(&status.appex_path))?;
    Ok((info.server_port()?, info.fs_type()?))
}

fn parse_statuses(stdout: &str) -> Vec<Status> {
    let election_re = Regex::new(r#"^\s*election = (\d+);$"#).unwrap();
    let path_re = Regex::new(r#"^\s*path = "([^"]+)";$"#).unwrap();

    let mut list = Vec::new();
    let mut elected = false;

    for line in stdout.lines() {
        if let Some(captures) = election_re.captures(line) {
            elected = &captures[1] == "1";
        }

        if let Some(captures) = path_re.captures(line) {
            list.push(Status {
                appex_path: PathBuf::from(&captures[1]),
                elected,
            });
            elected = false;
        }
    }

    list
}
