use std::process::exit;

use clap::Parser;
use exec_with_local_desktop::{
    args::{Args, Subcommands},
    client::client_main,
    gen_cert::gen_cert_main,
    server::server_main,
};

fn main() {
    let args = Args::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    match args.command {
        Subcommands::Client(args) => {
            let rst = rt.block_on(client_main(args));
            rt.shutdown_background(); // 不知道为什么会有 1 个 task 卡着, 只能强行关闭了.
            let code = rst.unwrap();
            if let Some(code) = code {
                exit(code);
            }
        }
        Subcommands::Server(args) => rt.block_on(server_main(args)).unwrap(),
        Subcommands::GenCert(args) => gen_cert_main(args),
    }
}
