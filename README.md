# SMRK–GUEGAP Probe v2.0 (ICP – Rust)

Status: ✅ WORKING (Local ICP Verified)

This repository contains a minimal deterministic on-chain screening prototype running on the Internet Computer (ICP).

The system has been successfully deployed and tested locally using `dfx`.

---

## What Is Implemented

Two-canister architecture:

- `registry_canister`
- `compute_canister`

### Flow

1. Job created in registry
2. Compute canister executes deterministic screening
3. Result written back on-chain
4. Registry returns stored output

---

## Verification Status

✔ Deployment successful  
✔ Canisters running  
✔ Job creation works  
✔ Screening execution works  
✔ Deterministic JSON result stored on-chain  
✔ Result retrieval verified  

Example output:

```json
{
  "probe_version": "smrk-guegap-icp-v2",
  "N": 256,
  "bulk_r": {
    "r_mean": 0.5815693,
    "gap": 0.22,
    "delta1": 0.0198043
  },
  "result": "pass"
}

How To Reproduce
dfx start --background --clean
dfx deploy

dfx canister call registry_canister create_job '(record { run_id="test-001"; input=blob "hello" })'
dfx canister call compute_canister run_screening '(record { run_id="test-001" })'
dfx canister call registry_canister get_job '(record { run_id="test-001" })'


---

## Local Deployment Snapshot (2026-02-21)

This section documents the current local deployment state
for `smrk-guegap-probe-v2.0-ICP-rust`.

> ⚠️ These IDs are valid only for the current local replica.
> They may change after `dfx deploy` or if `.dfx/` is removed.

---

### Local Canister IDs

- **compute_canister**  
  `uxrrr-q7777-77774-qaaaq-cai`

- **registry_canister**  
  `u6s2n-gx777-77774-qaaba-cai`

- **wallet (local)**  
  `uqqxf-5h777-77774-qaaaa-cai`

---

### Candid UI (Local)

- compute_canister  
  http://127.0.0.1:4943/?canisterId=uzt4z-lp777-77774-qaabq-cai&id=uxrrr-q7777-77774-qaaaq-cai

- registry_canister  
  http://127.0.0.1:4943/?canisterId=uzt4z-lp777-77774-qaabq-cai&id=u6s2n-gx777-77774-qaaba-cai

---

### Registry Audit Metadata (Verified)

```bash
dfx canister call registry_canister get_registry_meta

Output:

record {
  git_commit = "275a2dc4f80d";
  crate_version = "0.2.0";
  canister_version = 1 : nat64;
  build_ts = "2026-02-21";
}
