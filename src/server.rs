#![warn(clippy::all, clippy::pedantic)]
use clap::Parser;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Streaming;
use tonic::{Request, Response, Status, transport::Server};
use tracing::Level;

use crate::exec::execute_server::{Execute, ExecuteServer};
use crate::exec::{ExecuteRequestChunk, ProgramOutput};
use crate::server::executor::ProgramCaller;
use crate::{DEFAULT_PORT, SendStatus as _};

mod executor;

#[derive(clap::Parser)]
pub struct Args {
    #[clap(
        short = 'b',
        long = "bind",
        help = "Address the server bind to, recommend setting loopback address for safety.",
        default_value_t = format!("[::1]:{DEFAULT_PORT}")
    )]
    bind_address: String,
}

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
