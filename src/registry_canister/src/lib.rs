use candid::{CandidType, Deserialize};
use ic_cdk::api::{self, time};
use ic_cdk_macros::{post_upgrade, pre_upgrade, update};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap};
use serde_bytes::ByteBuf;
use sha2::{Digest, Sha256};

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum JobStatus {
    queued,
    running,
    done,
    failed,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct JobRecord {
    pub run_id: String,
    pub created_at_ns: u64,
    pub status: JobStatus,

    pub input: ByteBuf,
    pub input_sha256: ByteBuf,

    pub output: Option<ByteBuf>,
    pub output_sha256: Option<ByteBuf>, // 32 bytes
    pub math_sha256: Option<ByteBuf>,   // 32 bytes (NEW)
    pub commit_hash_hex: Option<String>,
    pub error: Option<String>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct RegistryMeta {
    pub git_commit: String,
    pub crate_version: String,
    pub canister_version: u64,
    pub build_ts: String,
}

#[derive(CandidType, Deserialize)]
pub struct SubmitJobReq {
    pub input: ByteBuf,
}

#[derive(CandidType, Deserialize)]
pub struct SubmitJobResp {
    pub run_id: String,
    pub run_id_hex: String,
}

#[derive(CandidType, Deserialize)]
pub struct GetJobReq {
    pub run_id: String,
}

#[derive(CandidType, Deserialize)]
pub struct MarkRunningReq {
    pub run_id: String,
}

#[derive(CandidType, Deserialize)]
pub struct SetResultReq {
    pub run_id: String,
    pub output: ByteBuf,
    pub math_sha256: ByteBuf, // 32 bytes (NEW)
}

#[derive(CandidType, Deserialize)]
pub struct SetFailedReq {
    pub run_id: String,
    pub error: String,
}

thread_local! {
    static MEMORY_MANAGER: std::cell::RefCell<MemoryManager<DefaultMemoryImpl>> =
        std::cell::RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static JOBS: std::cell::RefCell<StableBTreeMap<Vec<u8>, Vec<u8>, Memory>> =
        std::cell::RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0)))
        ));
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

fn encode_job(job: &JobRecord) -> Vec<u8> {
    candid::encode_one(job).expect("encode job")
}

fn decode_job(bytes: &[u8]) -> JobRecord {
    candid::decode_one(bytes).expect("decode job")
}

#[update]
fn submit_job(req: SubmitJobReq) -> SubmitJobResp {
    let created_at_ns = time();
    let input_hash = sha256(&req.input);
    let run_id = hex::encode(input_hash);

    let job = JobRecord {
        run_id: run_id.clone(),
        created_at_ns,
        status: JobStatus::queued,
        input: req.input.clone(),
        input_sha256: ByteBuf::from(input_hash.to_vec()),
        output: None,
        output_sha256: None,
        math_sha256: None,
        commit_hash_hex: None,
        error: None,
    };

    JOBS.with(|m| {
        m.borrow_mut()
            .insert(run_id.as_bytes().to_vec(), encode_job(&job))
    });

    SubmitJobResp {
        run_id: run_id.clone(),
        run_id_hex: run_id,
    }
}

// IMPORTANT: update (not query) so compute canister can call it reliably
#[update]
fn get_job(req: GetJobReq) -> Option<JobRecord> {
    JOBS.with(|m| {
        m.borrow()
            .get(&req.run_id.as_bytes().to_vec())
            .map(|v| decode_job(&v))
    })
}

#[update]
fn mark_running(req: MarkRunningReq) -> bool {
    let key = req.run_id.as_bytes().to_vec();

    JOBS.with(|m| {
        // 1) read phase
        let existing = m.borrow().get(&key);

        // 2) write phase
        if let Some(v) = existing {
            let mut job = decode_job(&v);
            if job.status == JobStatus::done || job.status == JobStatus::failed {
                return false;
            }
            job.status = JobStatus::running;

            m.borrow_mut().insert(key, encode_job(&job));
            true
        } else {
            false
        }
    })
}

#[update]
fn set_result(req: SetResultReq) -> bool {
    let key = req.run_id.as_bytes().to_vec();

    JOBS.with(|m| {
        let existing = m.borrow().get(&key);

        if let Some(v) = existing {
            let mut job = decode_job(&v);
            let out_hash = sha256(&req.output);

            job.status = JobStatus::done;
            job.output = Some(req.output);
            job.output_sha256 = Some(ByteBuf::from(out_hash.to_vec()));
            job.math_sha256 = Some(req.math_sha256);
            job.commit_hash_hex = Some(hex::encode(out_hash));
            job.error = None;

            m.borrow_mut().insert(key, encode_job(&job));
            true
        } else {
            false
        }
    })
}

#[update]
fn set_failed(req: SetFailedReq) -> bool {
    let key = req.run_id.as_bytes().to_vec();

    JOBS.with(|m| {
        let existing = m.borrow().get(&key);

        if let Some(v) = existing {
            let mut job = decode_job(&v);
            job.status = JobStatus::failed;
            job.error = Some(req.error);

            m.borrow_mut().insert(key, encode_job(&job));
            true
        } else {
            false
        }
    })
}

// IMPORTANT: update (not query) so compute canister can call it reliably
#[update]
fn get_registry_meta() -> RegistryMeta {
    RegistryMeta {
        git_commit: option_env!("GIT_COMMIT").unwrap_or("unknown").to_string(),
        crate_version: env!("CARGO_PKG_VERSION").to_string(),
        canister_version: api::canister_version(),
        build_ts: option_env!("BUILD_TS").unwrap_or("unknown").to_string(),
    }
}

#[pre_upgrade]
fn pre_upgrade_hook() {}

#[post_upgrade]
fn post_upgrade_hook() {}

ic_cdk::export_candid!();