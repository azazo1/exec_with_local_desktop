use crate::exec::execute_client::ExecuteClient;
use crate::exec::execute_request_chunk::RequestChunk;
use crate::exec::program_output::Payload;
use crate::exec::{Command, ExecuteRequestChunk, StderrChunk, StdoutChunk};
use crate::{DEFAULT_PORT, Error};
use clap::Parser as _;
use std::time::Duration;
use tonic::transport::Channel;

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
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
    #[clap(
        short = 'l',
        long = "leak",
        help = "leak the client when connection closed."
    )]
    leak: bool,
}

#[derive(bon::Builder)]
pub struct ExecuteOptions {
    executable: String,
    #[builder(required)]
    current_dir: Option<String>,
    args: Vec<String>,
    leak: bool,
}

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

    pub async fn execute(&mut self, execute_options: ExecuteOptions) -> Result<ExecuteOutput, Error> {
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

    async fn execute_stream() {
        // todo
    }
}
