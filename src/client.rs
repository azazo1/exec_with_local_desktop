use std::env;

use crate::Error;
use crate::args::ClientArgs;
use crate::exec::execute_client::ExecuteClient;
use crate::exec::execute_request_chunk::RequestChunk;
use crate::exec::program_output::Payload;
use crate::exec::{
    Command, ExecuteRequestChunk, ProgramOutput, StderrChunk, StdinChunk, StdoutChunk,
};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tonic::transport::Channel;
use tonic::{Status, Streaming};
use tracing::{debug, info, warn};

#[derive(bon::Builder)]
pub struct ExecuteOptions {
    executable: String,
    #[builder(required)]
    current_dir: Option<String>,
    args: Vec<String>,
    leak: bool,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct ExecuteOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: i32,
}

pub struct ExecutorClient {
    client: ExecuteClient<Channel>,
}

impl ExecutorClient {
    pub async fn connect(address: String) -> Result<ExecutorClient, Error> {
        let client = ExecuteClient::connect(address).await?;
        Ok(ExecutorClient { client })
    }

    pub async fn execute(
        &mut self,
        execute_options: ExecuteOptions,
    ) -> Result<ExecuteOutput, Error> {
        let input_stream = tokio_stream::once(ExecuteRequestChunk {
            request_chunk: Some(RequestChunk::Command(Command {
                executable: execute_options.executable,
                args: execute_options.args,
                current_dir: execute_options.current_dir,
                leak: execute_options.leak,
            })),
        });
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut code = -1i32;
        let stream = self.client.execute(input_stream).await?;
        let mut stream = stream.into_inner();
        while let Some(msg) = stream.message().await? {
            let Some(payload) = msg.payload else {
                continue;
            };
            match payload {
                Payload::StdoutChunk(StdoutChunk { mut data }) => {
                    stdout.append(&mut data);
                }
                Payload::StderrChunk(StderrChunk { mut data }) => {
                    stderr.append(&mut data);
                }
                Payload::ExitStatus(c) => code = c,
            }
        }
        Ok(ExecuteOutput {
            code,
            stderr,
            stdout,
        })
    }

    fn spawn_stdin_transmitter(
        &self,
        tx: Sender<ExecuteRequestChunk>,
        mut stdin: impl AsyncRead + Send + Unpin + 'static,
    ) -> JoinHandle<()> {
        let handle = tokio::spawn(async move {
            let mut buf = vec![0u8; 1024];
            loop {
                let len = stdin.read(&mut buf).await;
                let len = match len {
                    Ok(0) => {
                        debug!("stdin EOF");
                        // 发送空数据告知输入结束.
                        tx.send(ExecuteRequestChunk {
                            request_chunk: Some(RequestChunk::StdinChunk(StdinChunk {
                                data: vec![],
                            })),
                        })
                        .await
                        .ok();
                        break;
                    }
                    Ok(len) => len,
                    Err(e) => {
                        warn!("failed to read stdin: {:?}", e);
                        break;
                    }
                };
                if tx
                    .send(ExecuteRequestChunk {
                        request_chunk: Some(RequestChunk::StdinChunk(StdinChunk {
                            data: buf[..len].into(),
                        })),
                    })
                    .await
                    .is_err()
                {
                    warn!("receiver stream closed");
                    break;
                }
            }
        });
        debug!("stdin transmitter spawned");
        handle
    }

    async fn transmit_std_stream(
        &self,
        mut stream: Streaming<ProgramOutput>,
        mut stdout: impl AsyncWrite + Send + Unpin + 'static,
        mut stderr: impl AsyncWrite + Send + Unpin + 'static,
    ) -> Result<Option<i32>, Status> {
        let mut once_warn_stdout = Some(());
        let mut once_warn_stderr = Some(());
        while let Ok(msg) = stream.message().await {
            let Some(msg) = msg else {
                continue;
            };
            let Some(payload) = msg.payload else {
                continue;
            };
            match payload {
                Payload::ExitStatus(code) => return Ok(Some(code)),
                Payload::StderrChunk(chunk) => {
                    stderr
                        .write_all(&chunk.data)
                        .await
                        .inspect_err(|e| {
                            once_warn_stderr
                                .take()
                                .inspect(|_| warn!("failed to write stderr: {:?}", e));
                        })
                        .ok();
                    stderr.flush().await.ok();
                }
                Payload::StdoutChunk(chunk) => {
                    stdout
                        .write_all(&chunk.data)
                        .await
                        .inspect_err(|e| {
                            once_warn_stdout
                                .take()
                                .inspect(|_| warn!("failed to write stdout: {:?}", e));
                        })
                        .ok();
                    stdout.flush().await.ok();
                }
            }
        }
        Ok(None)
    }

    /// 流式执行程序, 返回程序的退出码, 当启动的程序 leak 了则可能没有返回码.
    pub async fn execute_stream(
        &mut self,
        execute_options: ExecuteOptions,
        stdin: impl AsyncRead + Send + Unpin + 'static,
        stdout: impl AsyncWrite + Send + Unpin + 'static,
        stderr: impl AsyncWrite + Send + Unpin + 'static,
    ) -> Result<Option<i32>, Error> {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        tx.send(ExecuteRequestChunk {
            request_chunk: Some(RequestChunk::Command(Command {
                executable: execute_options.executable,
                args: execute_options.args,
                current_dir: execute_options.current_dir,
                leak: execute_options.leak,
            })),
        })
        .await
        .unwrap();

        self.spawn_stdin_transmitter(tx.clone(), stdin);

        let resp = self
            .client
            .execute(tokio_stream::wrappers::ReceiverStream::new(rx))
            .await?;
        Ok(self
            .transmit_std_stream(resp.into_inner(), stdout, stderr)
            .await?)
    }
}

pub async fn client_main(args: ClientArgs) -> Result<Option<i32>, Error> {
    #[cfg(debug_assertions)]
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    let mut client = ExecutorClient::connect(args.server_address).await?;
    let result = client
        .execute_stream(
            ExecuteOptions::builder()
                .executable(args.executable)
                .current_dir(Some(args.current_dir.unwrap_or_else(|| {
                    env::current_dir()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into()
                })))
                .leak(args.leak)
                .args(args.args)
                .build(),
            tokio::io::stdin(),
            tokio::io::stdout(),
            tokio::io::stderr(),
        )
        .await?;
    info!("execute over: {result:?}");
    Ok(result)
}
