# RTCP(Rust TCP Reverse Proxy and TCP Tunneling)

## 文档

[English document](./readme.md)

## 简介

RTCP是一个基于Rust语言的高性能 TCP 反向代理和隧道工具，使用 tcp 池，进一步提升网络传输速度，为上层http服务提供更加稳定的传输性能。

## 支持特性

|特性名称|开发进度|
|--------|-------|
| 支持 http 首部解析 | ✅ |
| 支持 http 首部字段修改 | ✅ |
| 支持真实 ip 转发 | ✅ |
| 支持断连重试 | ✅ |
| 支持可视化界面 | 🚧开发中 |
| 支持多端口配置 | 🚧开发中 |
| 支持流量统计 | 🚧开发中 |
| 支持流量监控 | 🚧开发中 |
| 支持接口请求统计 | 🚧开发中 |




> 目前项目还在持续开发中，暂未稳定，请勿用于生产环境。

## 快速开始

### 安装

```bash
git clone https://github.com/xiaoxin-sky/rtcp
cd rtcp
cargo build --release
```

### 运行

```bash
# run server
./target/release/server
# run client
./target/release/client 
# run test backend server
./target/release/be
```

