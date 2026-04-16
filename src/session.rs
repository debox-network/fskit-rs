use std::path::Path;

use log::error;

use super::handler::Handler;
use super::info::Info;
use super::installer;
use super::mounter::Mounter;
use super::socket::Socket;
use super::{Filesystem, MountOptions, mounter, registration, socket};

use self::Error::ExtensionNotRegistered;

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

fn read_config(appex_id: &str) -> Result<(u16, String)> {
    let statuses = registration::registrations(appex_id)?;

    let Some(status) = statuses
        .iter()
        .find(|status| status.elected)
        .or_else(|| statuses.first())
    else {
        return Err(ExtensionNotRegistered);
    };

    let appex_path = installer::appex_path(&status.app_path)?;
    let info = Info::new(Path::new(&appex_path)).map_err(installer::Error::from)?;

    Ok((
        info.server_port().map_err(installer::Error::from)?,
        info.fs_type().map_err(installer::Error::from)?,
    ))
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

    #[error("FSKit extension is not registered")]
    ExtensionNotRegistered,
}
