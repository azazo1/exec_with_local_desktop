use clap::Parser as _;
use exec_with_local_desktop::exec::execute_client::ExecuteClient;
use exec_with_local_desktop::exec::execute_request_chunk::RequestChunk;
use exec_with_local_desktop::exec::program_output::Payload;
use exec_with_local_desktop::exec::{Command, ExecuteRequestChunk, StderrChunk, StdoutChunk};
use exec_with_local_desktop::{DEFAULT_PORT, Error, Result};
use std::path::{Path, PathBuf};
use tonic::async_trait;
use tonic::transport::Channel;

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(index = 1)]
    executable: String,
    #[clap(
        short = 'd',
        long = "current-dir",
        help = "the working directory of executable, default: where the executable is."
    )]
    current_dir: Option<String>,
    #[clap(index = 2, help = "the executable args")]
    args: Vec<String>,
    #[clap(short = 'a', long="address", default_value_t=format!("grpc://[::1]:{DEFAULT_PORT}"))]
    server_address: String,
}

#[derive(bon::Builder)]
struct ExecuteOptions {
    executable: String,
    #[builder(required)]
    current_dir: Option<String>,
    args: Vec<String>,
}

#[derive(Debug, Clone)]
struct ExecuteOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: i32,
}

struct Executor {
    client: ExecuteClient<Channel>,
}

impl Executor {
    async fn connect(address: String) -> Result<Executor> {
        let client = ExecuteClient::connect(address).await?;
        Ok(Executor { client })
    }

    async fn execute(&mut self, execute_options: ExecuteOptions) -> Result<ExecuteOutput> {
        let input_stream = tokio_stream::once(ExecuteRequestChunk {
            request_chunk: Some(RequestChunk::Command(Command {
                executable: execute_options.executable,
                args: execute_options.args,
                current_dir: execute_options.current_dir,
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

    async fn execute_stream() {
        // todo
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let mut exc = Executor::connect(args.server_address).await.unwrap();
    exc.execute(
        ExecuteOptions::builder()
            .args(args.args)
            .current_dir(args.current_dir)
            .executable(args.executable)
            .build(),
    )
    .await
    .unwrap();
}
