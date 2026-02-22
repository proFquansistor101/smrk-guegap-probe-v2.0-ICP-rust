# SMRK-GUEGAP Probe — ICP v2 (Rust CDK)

This repo is a **Rust-first learning implementation** of the v2 ICP-only screening architecture:

- `registry_canister` — on-chain job registry + audit log (`run_id = sha256(input)`, `commit_hash = sha256(output)`)
- `compute_canister` — pulls jobs from registry and commits a deterministic screening output

> Note: This Rust version currently ships a **deterministic screening stub** (like the Motoko repo),
> so you can learn the Rust CDK patterns (stable storage, candid, cross-canister calls).
> In the next iteration we can replace the stub with a real numeric kernel for **N=256**.

## Prerequisites

- DFINITY SDK (`dfx`)
- Rust toolchain (`rustup`)
- wasm target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

## Build & deploy (local)

```bash
dfx start --background
dfx deploy
```

## Test

1) Submit a job:

```bash
dfx canister call registry_canister submit_job '(record { input = blob "\7b\7d" })'
```

2) Run screening:

```bash
dfx canister call compute_canister run_screening '(record { run_id = "<RUN_ID_HEX>" })'
```

3) Read job:

```bash
dfx canister call registry_canister get_job '(record { run_id = "<RUN_ID_HEX>" })'
```

## Repo layout

```
.
├── dfx.json
├── candid
│   ├── registry.did
│   └── compute.did
└── src
    ├── registry_canister
    │   ├── Cargo.toml
    │   └── src/lib.rs
    └── compute_canister
        ├── Cargo.toml
        └── src/lib.rs
```

## Audit metadata

Each output JSON includes `meta.compute` and `meta.registry` (git commit, crate version, canister version, build_ts).
