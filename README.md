# exec_with_local_desktop

在 Windows 的 OpenSSH Server 上执行命令, 会导致无法使用图形界面程序的问题, 因为 sshd 是通过服务来启动的.

这个小工具在 Windows 本地桌面环境运行一个服务端, 然后在 ssh 中执行客户端, 与服务端同步输入输出流, 同时控制程序的启动和关闭.

## 安装方法

```shell
choco install protoc # 或者其他包管理器安装 protoc/protobuf
git clone https://github.com/azazo1/exec_with_local_desktop.git && cd exec_with_local_desktop
cargo install --path .
```

## 使用方法

设置开机自启命令, 来启动服务器:

```shell
rex s
```

在 ssh 中, 运行:

```shell
rex c
```

## tls 加密

生成根证书, 服务端证书和客户端证书 (使用自己的证书则可跳过这一步).

```shell
rex g
```

这样就能在用户配置目录 (`%USERPFOFILE%/.config/rex` / `~/.config/rex`) 下生成证书文件.
服务端会自动加载配置目录下的证书文件.

- 默认生成的证书文件有效期一年, 只能使用回环路径访问服务端.
