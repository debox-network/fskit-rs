use std::process::Output;

use log::error;

use super::handler::Handler;
use super::installer;
use super::mounter::Mounter;
use super::registration::read_config;
use super::socket::Socket;
use super::{Filesystem, MountOptions, mounter, socket};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Session {
    socket: Socket,
    mounter: Mounter,
}

impl Session {
    pub(super) async fn new<FS>(fs: FS, opts: MountOptions) -> Result<Self>
    where
        FS: Filesystem + Send + Sync + Clone + 'static,
    {
        let (server_port, fs_type) = read_config(&opts.fskit_id)?;

        let handler = Handler::new(fs);

        let socket = Socket::start(handler, server_port).await?;

        let mounter = match Mounter::mount(opts, &fs_type) {
            Ok(mount) => mount,
            Err(err) => {
                socket.stop().await;
                return Err(Error::Mounter(err));
            }
        };

        Ok(Self { socket, mounter })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.mounter.unmount().inspect_err(|err| error!("{err}"));

        futures::executor::block_on(async {
            self.socket.stop().await;
        });
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
    Installer(#[from] installer::Error),

    #[error(transparent)]
    Socket(#[from] socket::Error),

    #[error(transparent)]
    Mounter(#[from] mounter::Error),
}
