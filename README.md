# exec_with_local_desktop

在 Windows 的 OpenSSH Server 上执行命令, 会导致无法使用图形界面程序的问题, 因为 sshd 是通过服务来启动的.

这个小工具在 Windows 本地桌面环境运行一个服务端, 然后在 ssh 中执行客户端, 与服务端同步输入输出流, 同时控制程序的启动和关闭.

## 使用方法

```shell
cargo install --path .
```

设置开机自启命令, 来启动服务器:

```shell
rex s
```

在 ssh 中, 运行:

```shell
rex c 
```
