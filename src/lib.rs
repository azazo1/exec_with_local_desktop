use std::io;

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

pub type Result<T> = std::result::Result<T, Error>;
