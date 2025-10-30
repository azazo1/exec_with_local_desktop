use exec_with_local_desktop::client::{ExecuteOptions, ExecutorClient};
use exec_with_local_desktop::exec::execute_server::ExecuteServer;
use exec_with_local_desktop::server::Executor;
use rand::Rng;
use std::env;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tonic::transport::Server;
use tracing::info;

fn random_filename() -> String {
    let mut rng = rand::rng();
    let alphabet: Vec<char> = ('a'..='z').collect();
    (0..10)
        .map(|_| rng.random_range(..alphabet.len()))
        .map(|x| alphabet[x])
        .chain(".tmp".chars())
        .collect()
}

#[test]
fn test_random_filename() {
    let mut f = random_filename();
    f = dbg!(f);
    assert!(f.ends_with(".tmp"));
}

/// 测试服务端是否能够在 `leak = false` 时防止子进程泄露为孤儿进程.
#[cfg(unix)]
#[test]
fn no_leak() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    const ADDR: &str = "[::1]:23245";
    // server
    let s_join = thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            // 15 秒后退出.
            tokio::time::timeout(Duration::from_secs(10), async move {
                Server::builder()
                    .add_service(ExecuteServer::new(Executor))
                    .serve(ADDR.parse().unwrap())
                    .await
                    .unwrap();
            })
            .await
            .ok();
        });
        rt.shutdown_background();
    });
    thread::sleep(Duration::from_secs(1)); // 等待服务器先启动.
    let c_join = thread::spawn(|| {
        use std::{fs, time::Instant};

        let start_time = Instant::now();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let filename = random_filename();
        let filename_ = filename.clone();
        rt.block_on(async move {
            // 3 秒后退出
            tokio::time::timeout(Duration::from_secs(3), async move {
                let mut client = ExecutorClient::connect(format!("grpc://{ADDR}"))
                    .await
                    .unwrap();
                client
                    .execute(
                        ExecuteOptions::builder()
                            .executable("bash".into())
                            .current_dir(Some(env::current_dir().unwrap().to_string_lossy().into()))
                            .args(vec![
                                "-c".into(),
                                // 第 7 秒删除文件.
                                format!("touch {filename} && sleep 4 && rm {filename}"),
                            ])
                            .leak(false)
                            .build(),
                    )
                    .await
            })
            .await
            .unwrap_err();
        });
        rt.shutdown_background();
        info!("{:.2?} client runtime terminated", start_time.elapsed());

        // 第 6 秒查看文件是否被删除.
        thread::sleep(Duration::from_secs(2));
        assert!(Path::new(&filename_).is_file());
        fs::remove_file(filename_).unwrap();
        info!("{:.2?} client deleted tmp file", start_time.elapsed());
    });
    s_join.join().unwrap();
    c_join.join().unwrap();
}
