# spotter-network-producer
A Rust app that polls Spotter Network for weather reports, normalizes the event with the other `sigtor.org` loaders, and puts in storage.

## Running locally
- `cargo run`

## Testing
- `cargo test` to run unit tests

## Building
- `cargo fmt`
- `cargo clippy`
- `cargo build --release`
- `strip target/release/spotter-network-producer`

## TODO
- standardize config as a shared lib
