# open-parks

Is Retiro open? Madrid park alert status, served as SSR HTML and a JSON API.

## Development

The toolchain is pinned in `rust-toolchain.toml`; rustup installs it on first use.

```sh
cargo run
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

CI runs the last three on every push to `main` and on pull requests.
