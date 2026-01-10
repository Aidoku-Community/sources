# Zaimanhua Aidoku Source

This is a Rust-based Aidoku source for [Zaimanhua](https://www.zaimanhua.com), utilizing the `v4api` Mobile API to bypass web-based restrictions.

## Features
- **Search**: Supports title search.
- **Browse**: Popularity, Click, and other rankings.
- **Filters**: Sort by popularity, latest, etc.
- **Reading**: Native JSON API usage for fast chapter and page loading.

## Prerequisites
- Rust 1.60+
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- Aidoku CLI: `cargo install --git https://github.com/Aidoku/aidoku-rs aidoku-cli`

## Building
```bash
cargo build --target wasm32-unknown-unknown --release
```
or
```bash
aidoku build
```

## Note on Network
This source uses `v4api.zaimanhua.com` which generally bypasses simple WAF blocks found on the `www` subdomain. However, if you are in a restricted region, ensure your device has appropriate network access (VPN/Proxy) as Aidoku will use the device's connection.
