# port-mapping 端口映射

![Lastest commit (branch)](https://img.shields.io/github/last-commit/share121/port-mapping/master)
[![Test](https://github.com/share121/port-mapping/workflows/Test/badge.svg)](https://github.com/share121/port-mapping/actions)
[![Latest version](https://img.shields.io/crates/v/port-mapping.svg)](https://crates.io/crates/port-mapping)
![License](https://img.shields.io/crates/l/port-mapping.svg)

简单的映射端口程序

> **注意：** 只有 TCP 端口映射经过测试

## 使用

修改 mapping.txt 文件，格式如下：

```
# t+u 表示同时使用 tcp 和 udp 协议
# 把本地端口 40000-49999 映射到服务器 100.123.151.117 的端口 0000-9999 上
t+u 40000-49999 100.123.151.117:0000-9999

# 使用 tcp 协议，把本地端口 5666 映射到 localhost 的端口 80 上
tcp 5666 :80

# 使用 udp 协议，把本地端口 5666 映射到 localhost 的端口 80 上
udp 5666 :80
```

然后运行 port-mapping 即可
