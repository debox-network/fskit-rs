use std::path::PathBuf;

use log::error;
use regex::Regex;

use super::installer::{Error, PLUGINKIT, Result, run_cmd_out};

#[derive(Debug, Clone)]
pub(super) struct Status {
    pub app_path: PathBuf,
    pub elected: bool,
}

pub(super) fn registrations(appex_id: &str) -> Result<Vec<Status>> {
    let output = match run_cmd_out(PLUGINKIT, &["-m", "-i", appex_id, "--raw"]) {
        Ok(output) => output,
        Err(err) => {
            error!("failed to query pluginkit for {appex_id}: {err}");
            return Err(err);
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_statuses(&stdout)
}

fn parse_statuses(stdout: &str) -> Result<Vec<Status>> {
    let election_re = Regex::new(r#"^\s*election = (\d+);$"#).unwrap();
    let path_re = Regex::new(r#"^\s*path = "([^"]+)";$"#).unwrap();

    let mut list = Vec::new();
    let mut elected = false;

    for line in stdout.lines() {
        if let Some(captures) = election_re.captures(line) {
            elected = &captures[1] == "1";
        }

        if let Some(captures) = path_re.captures(line) {
            let appex_path = PathBuf::from(&captures[1]);
            let Some(app_path) = appex_path
                .parent()
                .and_then(|it| it.parent())
                .and_then(|it| it.parent())
            else {
                return Err(Error::InvalidExtensionPath);
            };
            list.push(Status {
                app_path: app_path.to_path_buf(),
                elected,
            });
            elected = false;
        }
    }

    Ok(list)
}
