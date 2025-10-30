use std::net::SocketAddr;

use crate::DEFAULT_PORT;
use clap::{Parser, Subcommand};

#[derive(Parser, PartialEq, Eq, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Subcommands,
}

#[derive(Subcommand, PartialEq, Eq, Debug)]
pub enum Subcommands {
    #[command(alias = "c")]
    Client(ClientArgs),
    #[command(alias = "s")]
    Server(ServerArgs),
}

#[derive(Parser, PartialEq, Eq, Debug)]
#[command(author, version, about = "run client", long_about = None)]
pub struct ClientArgs {
    // todo client shell 子命令
    #[clap(index = 1)]
    pub executable: String,
    #[clap(
        short = 'd',
        long = "current-dir",
        help = "the working directory of executable, default: where you execute the client."
    )]
    pub current_dir: Option<String>,
    #[clap(index = 2, help = "the executable args")]
    pub args: Vec<String>,
    #[clap(short = 'a', long="address", default_value_t=format!("grpc://[::1]:{DEFAULT_PORT}"))]
    pub server_address: String,
    #[clap(
        short = 'l',
        long = "leak",
        help = "leak the client when connection closed."
    )]
    pub leak: bool,
}

#[derive(Parser, PartialEq, Eq, Debug)]
#[command(author, version, about = "run server", long_about = None)]
pub struct ServerArgs {
    #[clap(
        short = 'b',
        long = "bind",
        help = "Address the server bind to, recommend setting loopback address for safety.",
        default_value_t = format!("[::1]:{DEFAULT_PORT}").parse().unwrap()
    )]
    pub bind_address: SocketAddr,
}

#[cfg(test)]
mod test {
    use crate::DEFAULT_PORT;
    use crate::args::{ClientArgs, ServerArgs, Subcommands};

    use super::Args;
    use clap::Parser as _;

    #[test]
    fn parse_client() {
        let raw_args = [
            env!("CARGO_PKG_NAME"),
            "client",
            "bash",
            "-d",
            "/usr/bin/",
            "-a",
            "grpc://nihao.com:5000",
            "--",
            "-c",
            "sleep 10",
        ]
        .iter();
        let args = Args::parse_from(raw_args);
        let target = Args {
            command: Subcommands::Client(ClientArgs {
                executable: "bash".into(),
                args: ["-c".into(), "sleep 10".into()].into(),
                current_dir: Some("/usr/bin/".into()),
                leak: false,
                server_address: "grpc://nihao.com:5000".into(),
            }),
        };

        assert_eq!(args, target);
    }

    #[test]
    fn parse_client_with_defaults() {
        let raw_args = [env!("CARGO_PKG_NAME"), "client", "ls"].iter();
        let args = Args::parse_from(raw_args);
        let target = Args {
            command: Subcommands::Client(ClientArgs {
                executable: "ls".into(),
                args: vec![],
                current_dir: None,
                leak: false,
                server_address: format!("grpc://[::1]:{DEFAULT_PORT}"),
            }),
        };
        assert_eq!(args, target);
    }

    #[test]
    fn parse_client_with_leak() {
        let raw_args = [
            env!("CARGO_PKG_NAME"),
            "client",
            "bash",
            "-l",
            "--",
            "-c",
            "echo hello",
        ]
        .iter();
        let args = Args::parse_from(raw_args);
        let target = Args {
            command: Subcommands::Client(ClientArgs {
                executable: "bash".into(),
                args: ["-c".into(), "echo hello".into()].into(),
                current_dir: None,
                leak: true,
                server_address: format!("grpc://[::1]:{}", DEFAULT_PORT),
            }),
        };
        assert_eq!(args, target);
    }

    #[test]
    fn parse_client_with_alias() {
        let raw_args = [env!("CARGO_PKG_NAME"), "c", "python3", "script.py"].iter();
        let args = Args::parse_from(raw_args);
        let target = Args {
            command: Subcommands::Client(ClientArgs {
                executable: "python3".into(),
                args: ["script.py".into()].into(),
                current_dir: None,
                leak: false,
                server_address: format!("grpc://[::1]:{}", DEFAULT_PORT),
            }),
        };
        assert_eq!(args, target);
    }

    #[test]
    fn parse_server() {
        let raw_args = [env!("CARGO_PKG_NAME"), "server", "-b", "[::1]:8080"].iter();
        let args = Args::parse_from(raw_args);
        let target = Args {
            command: Subcommands::Server(ServerArgs {
                bind_address: "[::1]:8080".parse().unwrap(),
            }),
        };
        assert_eq!(args, target);
    }

    #[test]
    fn parse_server_with_alias() {
        let raw_args = [env!("CARGO_PKG_NAME"), "s"].iter();
        let args = Args::parse_from(raw_args);
        let target = Args {
            command: Subcommands::Server(ServerArgs {
                bind_address: format!("[::1]:{}", DEFAULT_PORT).parse().unwrap(),
            }),
        };
        assert_eq!(args, target);
    }
}
