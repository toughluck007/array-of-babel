use rand::Rng;
use serde::{Deserialize, Serialize};

pub const GENERAL_TAG: &str = "GENERAL";
pub const SIMD_TAG: &str = "SIMD";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub name: String,
    pub tag: String,
    pub base_time_ms: u64,
    pub base_reward: u64,
    pub quality_target: u8,
    pub data_output: u64,
}

pub fn generate_general_job(id: u64, rng: &mut impl Rng) -> Job {
    let base_time_ms = rng.gen_range(4_000..9_000);
    let base_reward = rng.gen_range(70..140);
    let quality_target = rng.gen_range(55..85);
    let data_output = rng.gen_range(12..32);
    Job {
        id,
        name: format!("General Task #{id}"),
        tag: GENERAL_TAG.to_string(),
        base_time_ms,
        base_reward,
        quality_target,
        data_output,
    }
}

pub fn generate_simd_job(id: u64, rng: &mut impl Rng) -> Job {
    let base_time_ms = rng.gen_range(6_000..13_000);
    let base_reward = rng.gen_range(160..260);
    let quality_target = rng.gen_range(65..95);
    let data_output = rng.gen_range(36..72);
    Job {
        id,
        name: format!("SIMD Workload #{id}"),
        tag: SIMD_TAG.to_string(),
        base_time_ms,
        base_reward,
        quality_target,
        data_output,
    }
}

pub fn generate_job_with_tag(id: u64, tag: &str, rng: &mut impl Rng) -> Job {
    match tag {
        SIMD_TAG => generate_simd_job(id, rng),
        _ => generate_general_job(id, rng),
    }
}
