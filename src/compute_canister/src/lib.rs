use candid::{CandidType, Deserialize, Principal};
use ic_cdk::api;
use ic_cdk::api::call::call;
use ic_cdk_macros::{query, update};
use serde_bytes::ByteBuf;
use sha2::{Digest, Sha256};

// --- Public API ---

#[derive(CandidType, Deserialize)]
pub struct RunScreeningReq {
    pub run_id: String,
}

#[derive(CandidType, Deserialize)]
pub struct RunScreeningResp {
    pub ok: bool,
    pub message: String,
}

#[derive(CandidType, Deserialize)]
pub struct VerifyScreeningReq {
    pub run_id: String,
}

#[derive(CandidType, Deserialize)]
pub struct VerifyScreeningResp {
    pub ok: bool,
    pub matches: bool,
    pub message: String,
    pub stored_output_sha256_hex: Option<String>,
    pub computed_output_sha256_hex: Option<String>,
}

#[derive(CandidType, Deserialize)]
pub struct VerifyMathReq {
    pub run_id: String,
}

#[derive(CandidType, Deserialize)]
pub struct VerifyMathResp {
    pub ok: bool,
    pub matches: bool,
    pub message: String,
    pub stored_math_sha256_hex: Option<String>,
    pub computed_math_sha256_hex: Option<String>,
}

// --- Registry API subset ---

#[derive(CandidType, Deserialize)]
pub struct MarkRunningReq {
    pub run_id: String,
}

#[derive(CandidType, Deserialize)]
pub struct GetJobReq {
    pub run_id: String,
}

#[derive(CandidType, Deserialize, Clone)]
pub enum JobStatus {
    queued,
    running,
    done,
    failed,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct JobRecord {
    pub run_id: String,
    pub created_at_ns: u64,
    pub status: JobStatus,

    pub input: ByteBuf,
    pub input_sha256: ByteBuf,

    pub output: Option<ByteBuf>,
    pub output_sha256: Option<ByteBuf>,
    pub math_sha256: Option<ByteBuf>,
    pub commit_hash_hex: Option<String>,
    pub error: Option<String>,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct RegistryMeta {
    pub git_commit: String,
    pub crate_version: String,
    pub canister_version: u64,
    pub build_ts: String,
}

#[derive(CandidType, Deserialize)]
pub struct SetResultReq {
    pub run_id: String,
    pub output: ByteBuf,
    pub math_sha256: ByteBuf,
}

#[derive(CandidType, Deserialize)]
pub struct SetFailedReq {
    pub run_id: String,
    pub error: String,
}

// --- State ---

thread_local! {
    static REGISTRY: std::cell::RefCell<Option<Principal>> = std::cell::RefCell::new(None);
}

fn registry_principal_or_trap() -> Principal {
    REGISTRY.with(|r| {
        r.borrow()
            .clone()
            .unwrap_or_else(|| ic_cdk::trap("Registry principal not set. Call set_registry_canister(principal)."))
    })
}

#[update]
fn set_registry_canister(p: Principal) -> bool {
    REGISTRY.with(|r| *r.borrow_mut() = Some(p));
    true
}

#[query]
fn get_registry_canister() -> Option<Principal> {
    REGISTRY.with(|r| r.borrow().clone())
}

// --- Helpers ---

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

fn hex_bytes(b: &[u8]) -> String {
    hex::encode(b)
}

// Canonical math-only payload (NO meta) + its hash.
fn math_payload(input: &[u8], input_sha256: &[u8]) -> (Vec<u8>, [u8; 32]) {
    let h = sha256(input);

    let r_mean = (h[0] as f64 / 255.0) * 0.2 + 0.5;
    let delta1 = (h[1] as f64 / 255.0) * 0.05;
    let pass = if r_mean > 0.58 && delta1 > 0.005 { "pass" } else { "fail" };

    let input_sha_hex = hex_bytes(input_sha256);

    // stable key order + stable numeric formatting
    let json = format!(
        "{{\"probe_version\":\"smrk-guegap-icp-v2\",\"N\":256,\
\"bulk_r\":{{\"r_mean\":{:.16},\"count\":100}},\
\"gap\":{{\"delta1\":{:.16}}},\
\"H1_H2_proxy\":\"{}\",\
\"input_sha256_hex\":\"{}\"}}",
        r_mean,
        delta1,
        pass,
        input_sha_hex
    );

    let bytes = json.into_bytes();
    let hash = sha256(&bytes);
    (bytes, hash)
}

// Deterministic audit output derived from math + meta.
fn screening_output_audit_json(input: &[u8], input_sha256: &[u8], reg_meta: &RegistryMeta) -> (Vec<u8>, [u8; 32]) {
    let (_math_json_bytes, math_hash) = math_payload(input, input_sha256);
    let math_sha_hex = hex::encode(math_hash);

    let h = sha256(input);
    let r_mean = (h[0] as f64 / 255.0) * 0.2 + 0.5;
    let delta1 = (h[1] as f64 / 255.0) * 0.05;
    let pass = if r_mean > 0.58 && delta1 > 0.005 { "pass" } else { "fail" };

    let git_commit = option_env!("GIT_COMMIT").unwrap_or("unknown");
    let build_ts = option_env!("BUILD_TS").unwrap_or("unknown");
    let crate_version = env!("CARGO_PKG_VERSION");
    let canister_version = api::canister_version();

    let input_sha_hex = hex_bytes(input_sha256);

    let json = format!(
        "{{\"probe_version\":\"smrk-guegap-icp-v2\",\"N\":256,\
\"bulk_r\":{{\"r_mean\":{:.16},\"count\":100}},\
\"gap\":{{\"delta1\":{:.16}}},\
\"H1_H2_proxy\":\"{}\",\
\"meta\":{{\
\"input_sha256_hex\":\"{}\",\
\"math_sha256_hex\":\"{}\",\
\"compute\":{{\"git_commit\":\"{}\",\"crate_version\":\"{}\",\"canister_version\":{},\"build_ts\":\"{}\"}},\
\"registry\":{{\"git_commit\":\"{}\",\"crate_version\":\"{}\",\"canister_version\":{},\"build_ts\":\"{}\"}}\
}}}}",
        r_mean,
        delta1,
        pass,
        input_sha_hex,
        math_sha_hex,
        git_commit,
        crate_version,
        canister_version,
        build_ts,
        reg_meta.git_commit,
        reg_meta.crate_version,
        reg_meta.canister_version,
        reg_meta.build_ts
    );

    let bytes = json.into_bytes();
    let audit_hash = sha256(&bytes);
    (bytes, audit_hash)
}

#[update]
async fn run_screening(req: RunScreeningReq) -> RunScreeningResp {
    let registry = registry_principal_or_trap();

    // 1) mark running
    let (ok_run,): (bool,) = match call(
        registry,
        "mark_running",
        (MarkRunningReq { run_id: req.run_id.clone() },),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return RunScreeningResp {
                ok: false,
                message: format!("mark_running call failed: {:?}", e),
            }
        }
    };

    if !ok_run {
        return RunScreeningResp {
            ok: false,
            message: "Job not runnable (missing/done/failed).".to_string(),
        };
    }

    // 2) get job
    let (job_opt,): (Option<JobRecord>,) = match call(
        registry,
        "get_job",
        (GetJobReq { run_id: req.run_id.clone() },),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            let _ = call::<_, (bool,)>(
                registry,
                "set_failed",
                (SetFailedReq {
                    run_id: req.run_id.clone(),
                    error: format!("get_job call failed: {:?}", e),
                },),
            )
            .await;

            return RunScreeningResp {
                ok: false,
                message: format!("get_job call failed: {:?}", e),
            };
        }
    };

    let job = match job_opt {
        None => {
            let _ = call::<_, (bool,)>(
                registry,
                "set_failed",
                (SetFailedReq {
                    run_id: req.run_id.clone(),
                    error: "Job missing after mark_running.".to_string(),
                },),
            )
            .await;

            return RunScreeningResp {
                ok: false,
                message: "Job missing.".to_string(),
            };
        }
        Some(j) => j,
    };

    // 3) get registry meta
    let (reg_meta,): (RegistryMeta,) = match call(registry, "get_registry_meta", ()).await {
        Ok(v) => v,
        Err(_) => (
            RegistryMeta {
                git_commit: "unknown".to_string(),
                crate_version: "unknown".to_string(),
                canister_version: 0,
                build_ts: "unknown".to_string(),
            },
        ),
    };

    // 4) compute math hash + audit output
    let (_math_json, math_hash) = math_payload(&job.input, &job.input_sha256);
    let (audit_out, _audit_hash) =
        screening_output_audit_json(&job.input, &job.input_sha256, &reg_meta);

    // 5) set result
    let (ok_set,): (bool,) = match call(
        registry,
        "set_result",
        (SetResultReq {
            run_id: req.run_id.clone(),
            output: ByteBuf::from(audit_out),
            math_sha256: ByteBuf::from(math_hash.to_vec()),
        },),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            let _ = call::<_, (bool,)>(
                registry,
                "set_failed",
                (SetFailedReq {
                    run_id: req.run_id.clone(),
                    error: format!("set_result call failed: {:?}", e),
                },),
            )
            .await;

            return RunScreeningResp {
                ok: false,
                message: format!("set_result call failed: {:?}", e),
            };
        }
    };

    if ok_set {
        RunScreeningResp {
            ok: true,
            message: "Screening complete (stub) and committed.".to_string(),
        }
    } else {
        let _ = call::<_, (bool,)>(
            registry,
            "set_failed",
            (SetFailedReq {
                run_id: req.run_id.clone(),
                error: "Failed to set result.".to_string(),
            },),
        )
        .await;

        RunScreeningResp {
            ok: false,
            message: "Failed to commit output.".to_string(),
        }
    }
}

#[update]
async fn verify_screening(req: VerifyScreeningReq) -> VerifyScreeningResp {
    let registry = registry_principal_or_trap();

    let (job_opt,): (Option<JobRecord>,) = match call(
        registry,
        "get_job",
        (GetJobReq { run_id: req.run_id.clone() },),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return VerifyScreeningResp {
                ok: false,
                matches: false,
                message: format!("get_job call failed: {:?}", e),
                stored_output_sha256_hex: None,
                computed_output_sha256_hex: None,
            }
        }
    };

    let job = match job_opt {
        None => {
            return VerifyScreeningResp {
                ok: false,
                matches: false,
                message: "Job not found.".to_string(),
                stored_output_sha256_hex: None,
                computed_output_sha256_hex: None,
            }
        }
        Some(j) => j,
    };

    let stored_sha = match job.output_sha256.clone() {
        None => {
            return VerifyScreeningResp {
                ok: false,
                matches: false,
                message: "Job has no output_sha256 yet (not done?).".to_string(),
                stored_output_sha256_hex: None,
                computed_output_sha256_hex: None,
            }
        }
        Some(b) => b,
    };

    let (reg_meta,): (RegistryMeta,) = match call(registry, "get_registry_meta", ()).await {
        Ok(v) => v,
        Err(_) => (
            RegistryMeta {
                git_commit: "unknown".to_string(),
                crate_version: "unknown".to_string(),
                canister_version: 0,
                build_ts: "unknown".to_string(),
            },
        ),
    };

    let (computed_audit_out, _audit_hash) =
        screening_output_audit_json(&job.input, &job.input_sha256, &reg_meta);

    let computed_hash = sha256(&computed_audit_out);
    let computed_hex = hex::encode(computed_hash);

    let stored_hex = hex::encode(stored_sha.to_vec());
    let matches = stored_hex == computed_hex;

    VerifyScreeningResp {
        ok: true,
        matches,
        message: if matches {
            "Verified: computed output hash matches stored output_sha256.".to_string()
        } else {
            "Mismatch: computed output hash differs from stored output_sha256. (Tip: after upgrades, meta fields change and thus audit JSON hash changes.)".to_string()
        },
        stored_output_sha256_hex: Some(stored_hex),
        computed_output_sha256_hex: Some(computed_hex),
    }
}

#[update]
async fn verify_math(req: VerifyMathReq) -> VerifyMathResp {
    let registry = registry_principal_or_trap();

    let (job_opt,): (Option<JobRecord>,) = match call(
        registry,
        "get_job",
        (GetJobReq { run_id: req.run_id.clone() },),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return VerifyMathResp {
                ok: false,
                matches: false,
                message: format!("get_job call failed: {:?}", e),
                stored_math_sha256_hex: None,
                computed_math_sha256_hex: None,
            }
        }
    };

    let job = match job_opt {
        None => {
            return VerifyMathResp {
                ok: false,
                matches: false,
                message: "Job not found.".to_string(),
                stored_math_sha256_hex: None,
                computed_math_sha256_hex: None,
            }
        }
        Some(j) => j,
    };

    let stored = match job.math_sha256.clone() {
        None => {
            return VerifyMathResp {
                ok: false,
                matches: false,
                message: "Job has no math_sha256 yet (not done?). Run run_screening once on this job.".to_string(),
                stored_math_sha256_hex: None,
                computed_math_sha256_hex: None,
            }
        }
        Some(b) => b,
    };

    let (_math_json, math_hash) = math_payload(&job.input, &job.input_sha256);

    let stored_hex = hex::encode(stored.to_vec());
    let computed_hex = hex::encode(math_hash);

    let matches = stored_hex == computed_hex;

    VerifyMathResp {
        ok: true,
        matches,
        message: if matches {
            "Verified: math_sha256 matches.".to_string()
        } else {
            "Mismatch: math_sha256 differs.".to_string()
        },
        stored_math_sha256_hex: Some(stored_hex),
        computed_math_sha256_hex: Some(computed_hex),
    }
}

ic_cdk::export_candid!();
