use crate::sim::jobs::Job;
use crate::sim::processors::ProcessorState;
use rand::Rng;

pub fn assignment_duration_ms(job: &Job, processor: &ProcessorState, daemon_penalty: bool) -> u64 {
    let base = job.base_time_ms as f64;
    let mut duration = base / processor.speed.max(0.1);
    if daemon_penalty {
        duration *= 1.1;
    }
    duration.round().max(1.0) as u64
}

pub fn roll_quality(
    job: &Job,
    processor: &ProcessorState,
    daemon_penalty: bool,
    rng: &mut impl Rng,
) -> u8 {
    let noise: i8 = rng.gen_range(-4..=4);
    let mut quality = job.quality_target as i16 + processor.quality_bias as i16 + noise as i16;
    if daemon_penalty {
        quality -= 5;
    }
    quality.clamp(0, 100) as u8
}

pub fn payout_for_quality(job: &Job, quality: u8) -> u64 {
    let factor = 0.7 + (quality as f64 / 100.0) * 0.5;
    ((job.base_reward as f64) * factor).round() as u64
}

pub fn upkeep_total(processors: &[ProcessorState]) -> u64 {
    processors.iter().map(|p| p.upkeep_cost).sum()
}

pub fn passive_income(stored_data: u64) -> u64 {
    if stored_data == 0 {
        0
    } else {
        (((stored_data as f64) * 0.05).round() as u64).max(1)
    }
}
