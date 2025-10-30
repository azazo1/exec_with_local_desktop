use std::io;

use tokio::sync::mpsc::Sender;
use tonic::Status;

use crate::exec::ProgramOutput;

pub mod client;
pub mod server;

pub mod exec {
    tonic::include_proto!("exec");
}

pub const DEFAULT_PORT: u16 = 30521;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    IOError(#[from] io::Error),
    #[error("{0}")]
    TonicTransportError(#[from] tonic::transport::Error),
    #[error("{0}")]
    TonicStatus(#[from] tonic::Status),
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
