# port-mapping 端口映射

![Lastest commit (branch)](https://img.shields.io/github/last-commit/share121/port-mapping/master)
[![Test](https://github.com/share121/port-mapping/workflows/Test/badge.svg)](https://github.com/share121/port-mapping/actions)
[![Latest version](https://img.shields.io/crates/v/port-mapping.svg)](https://crates.io/crates/port-mapping)
![License](https://img.shields.io/crates/l/port-mapping.svg)

简单的映射端口程序，有基础的负载均衡功能

> **注意：** 只有 TCP 端口映射经过测试

## 使用

修改 mapping.txt 文件，格式如下：

```
:80           :8080            tcp # 把 0.0.0.0:80 端口映射到 localhost:8080 端口，协议为 tcp
:80           :8081            tcp # 把 0.0.0.0:80 端口映射到 localhost:8081 端口，协议为 tcp，负载均衡
127.0.0.1:443 100.88.11.5:8080 tcp # 把 127.0.0.1:443 端口映射到 100.88.11.5:8080 端口，协议为 tcp
```

然后运行 port-mapping 即可
