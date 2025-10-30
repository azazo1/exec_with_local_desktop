use exec_with_local_desktop::exec::execute_request_chunk::RequestChunk;
use exec_with_local_desktop::exec::program_output::Payload;
use exec_with_local_desktop::exec::{StderrChunk, StdoutChunk};
use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio_stream::StreamExt;
use tonic::Streaming;

use clap::Parser;
use exec_with_local_desktop::DEFAULT_PORT;
use exec_with_local_desktop::exec::{
    ExecuteRequestChunk, ProgramOutput,
    execute_server::{Execute, ExecuteServer},
};
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};
use tracing::Level;

#[derive(clap::Parser)]
struct Args {
    #[clap(
        short = 'b',
        long = "bind",
        help = "Address the server bind to, recommend setting loopback address for safety.",
        default_value_t = format!("[::1]:{DEFAULT_PORT}")
    )]
    bind_address: String,
}

struct Executor;

async fn call_program(
    mut request: Streaming<ExecuteRequestChunk>,
    tx: Sender<Result<ProgramOutput, Status>>,
) {
    let Ok(Some(ExecuteRequestChunk {
        request_chunk: Some(RequestChunk::Command(command)),
    })) = request.message().await
    else {
        tx.send(Err(Status::invalid_argument(
            "can not get command in first chunk",
        )))
        .await
        .ok();
        return;
    };
    let Ok(exec_path) = which::which(PathBuf::from(command.executable)) else {
        tx.send(Err(Status::not_found("executable not found")))
            .await
            .ok();
        return;
    };
    if exec_path.is_relative() {
        tx.send(Err(Status::invalid_argument(
            "relative executable path is not supported",
        )))
        .await
        .ok();
        return;
    }
    let mut cmd = Command::new(&exec_path);
    let current_dir = match command.current_dir {
        Some(it) => it,
        None => {
            if let Some(dir) = exec_path.parent() {
                dir.as_os_str().to_string_lossy().to_string()
            } else {
                tx.send(Err(Status::not_found(
                    "cannot set current dir automatically",
                )))
                .await
                .ok();
                return;
            }
        }
    };
    let mut child = match cmd
        .args(command.args)
        .current_dir(current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            tx.send(Err(Status::unknown(e.to_string()))).await.ok();
            return;
        }
    };

    let stderr = child.stderr.take().unwrap();
    let tx1 = tx.clone();
    tokio::spawn(async move {
        let mut br = BufReader::new(stderr);
        let mut buf = vec![0u8; 1024];
        while let Ok(read_len) = br.read(&mut buf).await {
            if tx1.send(Ok(ProgramOutput {
                payload: Some(Payload::StderrChunk(StderrChunk {
                    data: buf[..read_len].to_vec(),
                })),
            }))
            .await
            .is_err() {
                break;
            }
        }
    });

    let stdout = child.stdout.take().unwrap();
    let tx1 = tx.clone();
    tokio::spawn(async move {
        let mut br = BufReader::new(stdout);
        let mut buf = vec![0u8; 1024];
        while let Ok(read_len) = br.read(&mut buf).await {
            if tx1
                .send(Ok(ProgramOutput {
                    payload: Some(Payload::StdoutChunk(StdoutChunk {
                        data: buf[..read_len].to_vec(),
                    })),
                }))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let mut stdin = child.stdin.take().unwrap();
    while let Ok(request_chunk) = request.message().await {
        let Some(ExecuteRequestChunk {
            request_chunk: Some(request_chunk),
        }) = request_chunk
        else {
            continue;
        };
        match request_chunk {
            RequestChunk::StdinChunk(stdin_chunk) => {
                match stdin.write_all(&stdin_chunk.data).await {
                    Ok(_) => (),
                    Err(e) => {
                        tx.send(Err(Status::internal(e.to_string()))).await.ok();
                    }
                }
            }
            RequestChunk::Kill(_) => {
                child.kill().await.ok();
                match child.wait().await {
                    Ok(status) => {
                        tx.send(Ok(ProgramOutput {
                            payload: Some(Payload::ExitStatus(status.code().unwrap_or(-1))),
                        }))
                        .await
                        .ok();
                    }
                    Err(e) => {
                        tx.send(Err(Status::internal(e.to_string()))).await.ok();
                    }
                }
                break;
            }
            _ => {}
        }
    }
}

#[tonic::async_trait]
impl Execute for Executor {
    type executeStream = ReceiverStream<Result<ProgramOutput, Status>>;
    async fn execute(
        &self,
        req: Request<Streaming<ExecuteRequestChunk>>,
    ) -> Result<Response<Self::executeStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        tokio::spawn(async move {
            call_program(req.into_inner(), tx).await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    let addr = Args::parse().bind_address.parse().unwrap();
    Server::builder()
        .add_service(ExecuteServer::new(Executor))
        .serve(addr)
        .await
        .unwrap();
}
