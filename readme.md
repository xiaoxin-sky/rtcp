# RTCP (Rust TCP Reverse Proxy and TCP Tunneling)

## Documentation

[中文文档](./readme-zh.md)

## Introduction

RTCP is a high-performance TCP reverse proxy and tunneling tool based on the Rust language. It utilizes a TCP pool to further enhance network transmission speed, providing more stable transmission performance for upper-layer HTTP services.

## Supported Features

| Feature                | Development Status |
|------------------------|--------------------|
| HTTP header parsing    | ✅                 |
| HTTP header modification | ✅               |
| Real IP forwarding     | ✅                 |
| Connection retry on disconnect | ✅        |
| Visual interface       | 🚧 (In progress)   |
| Multi-port configuration | 🚧 (In progress) |
| Traffic statistics     | 🚧 (In progress)   |
| Traffic monitoring     | 🚧 (In progress)   |
| API request statistics | 🚧 (In progress)   |

> The project is still under active development and is not yet stable. Please do not use it in production environments.

## Quick Start

### Installation

```bash
git clone https://github.com/xiaoxin-sky/rtcp
cd rtcp
cargo build --release
```

### Run

```bash
# run server
./target/release/server
# run client
./target/release/client 
# run test backend server
./target/release/be
```


