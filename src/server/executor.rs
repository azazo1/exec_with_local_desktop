#![warn(clippy::all, clippy::pedantic)]

use std::{path::PathBuf, process::Stdio, time::Duration};

use crate::exec::{
    ExecuteRequestChunk, ProgramOutput, StderrChunk, StdoutChunk,
    execute_request_chunk::RequestChunk, program_output::Payload,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _, BufReader},
    process::{Child, ChildStderr, ChildStdout, Command},
    sync::mpsc::Sender,
    task::JoinHandle,
};
use tonic::{Status, Streaming};
use tracing::debug;

pub struct ProgramCaller {
    executable: PathBuf,
    current_dir: PathBuf,
    args: Vec<String>,
    leak: bool,
    output_sender: Sender<Result<ProgramOutput, Status>>,
    request_stream: Streaming<ExecuteRequestChunk>,
}

impl ProgramCaller {
    fn spawn_stdout_transmitter(&self, stdout: ChildStdout) -> JoinHandle<()> {
        let tx = self.output_sender.clone();
        let handle = tokio::spawn(async move {
            let mut br = BufReader::new(stdout);
            let mut buf = vec![0u8; 1024];
            while let Ok(read_len) = br.read(&mut buf).await {
                if read_len == 0 {
                    break;
                }
                if tx
                    .send(Ok(ProgramOutput {
                        payload: Some(Payload::StdoutChunk(StdoutChunk {
                            data: buf[..read_len].to_vec(),
                        })),
                    }))
                    .await
                    .is_err()
                {
                    debug!("stdout closed");
                    break;
                }
            }
        });
        debug!("stdout transmitter spawned");
        handle
    }

    fn spawn_stderr_transmitter(&self, stderr: ChildStderr) -> JoinHandle<()> {
        let tx = self.output_sender.clone();
        let handle = tokio::spawn(async move {
            let mut br = BufReader::new(stderr);
            let mut buf = vec![0u8; 1024];
            while let Ok(read_len) = br.read(&mut buf).await {
                if read_len == 0 {
                    break;
                }
                if tx
                    .send(Ok(ProgramOutput {
                        payload: Some(Payload::StderrChunk(StderrChunk {
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

        debug!("stderr transmitter spawned");
        handle
    }

    /// [`child`] 的 [`ChildStdin`] 不能为 [`None`], 否则 panic.
    async fn transmit_stdin(&mut self, mut child: Child) -> Result<Child, Status> {
        let mut stdin = child.stdin.take().unwrap();
        while let Ok(request_chunk) = tokio::select! {
           msg = self.request_stream.message() => msg,
           () = tokio::time::sleep(Duration::from_secs_f32(1.0)) => Ok(None),
        } {
            // debug!("nihc"); // todo 解决疯狂循环(间隔很短, 没有预想的 1 秒)的问题.
            let Some(ExecuteRequestChunk {
                request_chunk: Some(request_chunk),
            }) = request_chunk
            else {
                // 每隔一段时间检查一下子进程是否已经退出.
                match child.try_wait() {
                    Ok(Some(_)) => {
                        // 进程结束的信息留给外面的函数报告.
                        break;
                    }
                    Err(e) => {
                        // 无法获取子进程信息.
                        return Err(Status::internal(e.to_string()));
                    }
                    _ => continue,
                }
            };
            match request_chunk {
                RequestChunk::StdinChunk(stdin_chunk) => {
                    if stdin_chunk.data.is_empty() {
                        debug!("stdin EOF");
                        break;
                    }
                    match stdin.write_all(&stdin_chunk.data).await {
                        Ok(()) => (),
                        Err(e) => {
                            return Err(Status::internal(e.to_string()));
                        }
                    }
                }
                RequestChunk::Kill(_) => {
                    child.kill().await.ok();
                    match child.wait().await {
                        Ok(status) => {
                            self.output_sender
                                .send(Ok(ProgramOutput {
                                    payload: Some(Payload::ExitStatus(status.code().unwrap_or(-1))),
                                }))
                                .await
                                .ok();
                        }
                        Err(e) => {
                            return Err(Status::internal(e.to_string()));
                        }
                    }
                    break;
                }
                RequestChunk::Command(_) => {}
            }
        }
        Ok(child)
    }

    /// 根据字段中的启动信息来启动进程, 如果发生错误,
    /// 那么错误 [`Status`] 会通过返回值提供, 不会在 [`Sender`] 中发送.
    ///
    /// 当连接在程序执行完毕前终止时, 如果 [`ProgramCaller::leak`] 属性设置为 false,
    /// 那么程序会被 [`Child::kill`] 命令杀死, 否则不会, 需要手动将子程序杀死.
    ///
    /// 当连接未被关闭时, 此方法可被调用多次.
    ///
    /// # Returns
    /// 当执行正常时, 返回子程序对象; 当出现错误时, 返回 [`Status`] 错误信息.
    pub async fn call_program(&mut self) -> Result<Child, Status> {
        let mut child = match Command::new(&self.executable)
            .args(&self.args)
            .current_dir(&self.current_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                return Err(Status::unknown(e.to_string()));
            }
        };

        debug!("child spawn");

        self.spawn_stderr_transmitter(child.stderr.take().unwrap());
        self.spawn_stdout_transmitter(child.stdout.take().unwrap());
        let mut child = self.transmit_stdin(child).await?;
        if !self.leak {
            debug!("kill sub process: {:?}", child.kill().await);
        }

        if let Ok(Some(es)) = child.try_wait() {
            debug!("sub process exited: {:?}", es.code());
            self.output_sender
                .send(Ok(ProgramOutput {
                    payload: Some(Payload::ExitStatus(es.code().unwrap_or(-1))),
                }))
                .await
                .ok();
        } else {
            debug!("sub process leaked.");
        }
        Ok(child)
    }

    /// 从输入请求流中解析进程启动信息, 如果发生了错误, 返回 [`Status`] 错误信息, 不会向 tx 中发送 [`Err`].
    pub async fn parse(
        mut request: Streaming<ExecuteRequestChunk>,
        tx: Sender<Result<ProgramOutput, Status>>,
    ) -> Result<ProgramCaller, Status> {
        let Ok(Some(ExecuteRequestChunk {
            request_chunk: Some(RequestChunk::Command(command)),
        })) = request.message().await
        else {
            return Err(Status::invalid_argument(
                "can not get command in first chunk",
            ));
        };
        let Ok(executable) = which::which(PathBuf::from(command.executable)) else {
            return Err(Status::not_found("executable not found"));
        };
        if executable.is_relative() {
            return Err(Status::invalid_argument(
                "relative executable path is not supported",
            ));
        }
        let current_dir = match command.current_dir {
            Some(it) => it,
            None => {
                if let Some(dir) = executable.parent() {
                    dir.as_os_str().to_string_lossy().to_string()
                } else {
                    return Err(Status::not_found("cannot set current dir automatically"));
                }
            }
        };
        Ok(ProgramCaller {
            current_dir: current_dir.into(),
            leak: command.leak,
            args: command.args,
            output_sender: tx,
            request_stream: request,
            executable,
        })
    }
}
