use std::{env, io, path::PathBuf};

use tokio::sync::mpsc::Sender;
use tonic::Status;

use crate::exec::ProgramOutput;

pub mod args;
pub mod client;
pub mod gen_cert;
pub mod server;

pub mod exec {
    #![allow(non_camel_case_types)]
    tonic::include_proto!("exec");
}

const CA_CERT: &str = "ca_cert.crt";
const SERVER_CERT: &str = "server_cert.crt";
const SERVER_SECRET: &str = "server_secret.pem";
const CLIENT_CERT: &str = "client_cert.crt";
const CLIENT_SECRET: &str = "client_secret.pem";

pub const DEFAULT_PORT: u16 = 30521;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    IOError(#[from] io::Error),
    #[error("{0}")]
    TonicTransportError(#[from] tonic::transport::Error),
    #[error("{0}")]
    TonicStatus(#[from] tonic::Status),
    #[error("{0}")]
    EnvVarError(#[from] env::VarError),
}

pub trait SendStatus {
    type Inner;
    #[allow(async_fn_in_trait)]
    async fn send_status(self, tx: Sender<Result<ProgramOutput, Status>>) -> Option<Self::Inner>;
}

impl<T> SendStatus for Result<T, Status> {
    type Inner = T;
    async fn send_status(self, tx: Sender<Result<ProgramOutput, Status>>) -> Option<Self::Inner> {
        match self {
            Ok(i) => Some(i),
            Err(e) => {
                tx.send(Err(e)).await.ok();
                None
            }
        }
    }
}

pub fn config_dir() -> Result<PathBuf, Error> {
    let home: PathBuf = env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?.into();
    let config = home.join(".config").join("rex");
    Ok(config)
}
