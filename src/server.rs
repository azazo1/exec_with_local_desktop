use std::time::Duration;

use tokio::fs;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Streaming;
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};
use tonic::{Request, Response, Status};

use crate::args::ServerArgs;
use crate::exec::execute_server::{Execute, ExecuteServer};
use crate::exec::{ExecuteRequestChunk, ProgramOutput};
use crate::server::executor::ProgramCaller;
use crate::{CA_CERT, CLIENT_CERT, Error, config_dir};
use crate::{SERVER_CERT, SERVER_SECRET, SendStatus as _};

mod executor;

pub struct Executor;

#[tonic::async_trait]
impl Execute for Executor {
    type executeStream = ReceiverStream<Result<ProgramOutput, Status>>;
    async fn execute(
        &self,
        req: Request<Streaming<ExecuteRequestChunk>>,
    ) -> Result<Response<Self::executeStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(30);
        tokio::spawn(async move {
            let Some(mut pc) = ProgramCaller::parse(req.into_inner(), tx.clone())
                .await
                .send_status(tx.clone())
                .await
            else {
                return;
            };
            let _ = pc.call_program().await.send_status(tx.clone()).await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

pub async fn server_main(args: ServerArgs) -> Result<(), Error> {
    if cfg!(debug_assertions) {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }
    let cert_dir = args.cert_dir.unwrap_or(config_dir()?);
    let server_cert = fs::read(cert_dir.join(SERVER_CERT)).await?;
    let server_secret = fs::read(cert_dir.join(SERVER_SECRET)).await?;
    let tls_config = ServerTlsConfig::new()
        .client_ca_root(Certificate::from_pem(
            fs::read(cert_dir.join(CA_CERT)).await?,
        ))
        .identity(Identity::from_pem(server_cert, server_secret))
        .timeout(Duration::from_secs(1));
    Ok(Server::builder()
        .tls_config(tls_config)?
        .add_service(ExecuteServer::new(Executor))
        .serve(args.bind_address)
        .await?)
}
