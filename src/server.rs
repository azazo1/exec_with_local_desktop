use tokio_stream::wrappers::ReceiverStream;
use tonic::Streaming;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::Error;
use crate::SendStatus as _;
use crate::args::ServerArgs;
use crate::exec::execute_server::{Execute, ExecuteServer};
use crate::exec::{ExecuteRequestChunk, ProgramOutput};
use crate::server::executor::ProgramCaller;

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
    Ok(Server::builder()
        .add_service(ExecuteServer::new(Executor))
        .serve(args.bind_address)
        .await?)
}
