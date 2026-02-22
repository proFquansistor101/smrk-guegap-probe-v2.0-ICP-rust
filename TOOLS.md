## Tooling note: ic-wasm

`dfx.json` uses `ic-wasm ... shrink` to reduce Wasm size.

Install (one option):
- `cargo install ic-wasm`

If you don't want shrinking, remove the `ic-wasm ... shrink` part from the build commands in `dfx.json`.
