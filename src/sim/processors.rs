use crate::sim::jobs::Job;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

const DEFAULT_RELIABILITY: f64 = 0.995;
const DEFAULT_COOLING_CAP: u8 = 3;
const DEFAULT_REPLACE_RATIO: f64 = 0.35;
const DEFAULT_POWER_DRAW: f64 = 4.2;
const DEFAULT_HEAT_OUTPUT: f64 = 1.0;
const DEFAULT_PURCHASE_COST: u64 = 180;
const HEAT_FAILURE_MULTIPLIER: f64 = 0.12;
const ELECTRIC_COOLING_FACTOR: f64 = 0.05;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonPenalty {
    pub quality: i8,
    pub time_multiplier: f64,
}

impl Default for DaemonPenalty {
    fn default() -> Self {
        Self {
            quality: -5,
            time_multiplier: 1.10,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DaemonMode {
    Off,
    Assist,
    Auto,
}

impl Default for DaemonMode {
    fn default() -> Self {
        DaemonMode::Off
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorWork {
    pub job: Job,
    pub remaining_ms: u64,
    pub total_ms: u64,
    pub daemon_penalty: Option<DaemonPenalty>,
    #[serde(default)]
    pub overheating: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessorStatus {
    Idle,
    Working(Box<ProcessorWork>),
    BurntOut,
    Destroyed,
}

impl Default for ProcessorStatus {
    fn default() -> Self {
        ProcessorStatus::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorState {
    pub name: String,
    pub speed: f64,
    pub quality_bias: i8,
    pub instruction_set: Vec<String>,
    pub upkeep_cost: u64,
    #[serde(default)]
    pub status: ProcessorStatus,
    #[serde(default = "default_reliability_base")]
    pub reliability_base: f64,
    #[serde(default)]
    pub cooling_required: bool,
    #[serde(default)]
    pub cooling_level: u8,
    #[serde(default = "default_cooling_cap")]
    pub cooling_cap: u8,
    #[serde(default)]
    pub hardening_level: u8,
    #[serde(default)]
    pub requires_cooling_min: u8,
    #[serde(default)]
    pub finite_lifespan: bool,
    #[serde(default)]
    pub mttf_ticks: u64,
    #[serde(default)]
    pub wear: f64,
    #[serde(default)]
    pub fragility: f64,
    #[serde(default = "default_replace_cost_ratio")]
    pub replace_cost_ratio: f64,
    #[serde(default = "default_power_draw_base")]
    pub power_draw_base: f64,
    #[serde(default)]
    pub power_draw_mod: HashMap<String, f64>,
    #[serde(default = "default_heat_output_base")]
    pub heat_output_base: f64,
    #[serde(default = "default_purchase_cost")]
    pub purchase_cost: u64,
    #[serde(default)]
    pub daemon_mode: DaemonMode,
    #[serde(default)]
    pub daemon_unlocked: bool,
    #[serde(default)]
    pub daemon_affinity: HashMap<String, f64>,
    #[serde(default)]
    pub daemon_priority: i32,
    #[serde(default = "default_honor_cooling")]
    pub honor_cooling_mins: bool,
    #[serde(default)]
    pub daemon_penalty: DaemonPenalty,
    #[serde(skip)]
    pub last_reliability: f64,
    #[serde(skip)]
    pub last_heat: f64,
    #[serde(skip)]
    pub last_power_draw: f64,
    #[serde(skip)]
    pub last_effective_cooling: u8,
}

fn default_reliability_base() -> f64 {
    DEFAULT_RELIABILITY
}

fn default_cooling_cap() -> u8 {
    DEFAULT_COOLING_CAP
}

fn default_replace_cost_ratio() -> f64 {
    DEFAULT_REPLACE_RATIO
}

fn default_power_draw_base() -> f64 {
    DEFAULT_POWER_DRAW
}

fn default_heat_output_base() -> f64 {
    DEFAULT_HEAT_OUTPUT
}

fn default_purchase_cost() -> u64 {
    DEFAULT_PURCHASE_COST
}

fn default_honor_cooling() -> bool {
    true
}

impl ProcessorState {
    pub fn starter() -> Self {
        let mut processor = Self {
            name: "Model F12-Scalar".to_string(),
            speed: 1.0,
            quality_bias: 0,
            instruction_set: vec!["GENERAL".to_string()],
            upkeep_cost: 8,
            status: ProcessorStatus::Idle,
            reliability_base: DEFAULT_RELIABILITY,
            cooling_required: false,
            cooling_level: 0,
            cooling_cap: DEFAULT_COOLING_CAP,
            hardening_level: 0,
            requires_cooling_min: 0,
            finite_lifespan: false,
            mttf_ticks: 0,
            wear: 0.0,
            fragility: 0.0,
            replace_cost_ratio: DEFAULT_REPLACE_RATIO,
            power_draw_base: DEFAULT_POWER_DRAW,
            power_draw_mod: HashMap::new(),
            heat_output_base: DEFAULT_HEAT_OUTPUT,
            purchase_cost: DEFAULT_PURCHASE_COST,
            daemon_mode: DaemonMode::Off,
            daemon_unlocked: false,
            daemon_affinity: HashMap::new(),
            daemon_priority: 0,
            honor_cooling_mins: true,
            daemon_penalty: DaemonPenalty::default(),
            last_reliability: DEFAULT_RELIABILITY,
            last_heat: 0.0,
            last_power_draw: DEFAULT_POWER_DRAW,
            last_effective_cooling: 0,
        };
        processor.ensure_runtime_defaults();
        processor
    }

    pub fn ensure_runtime_defaults(&mut self) {
        if self.cooling_cap == 0 {
            self.cooling_cap = DEFAULT_COOLING_CAP;
        }
        if self.replace_cost_ratio == 0.0 {
            self.replace_cost_ratio = DEFAULT_REPLACE_RATIO;
        }
        if self.reliability_base <= 0.0 {
            self.reliability_base = DEFAULT_RELIABILITY;
        }
        if (self.power_draw_base - 0.0).abs() < f64::EPSILON {
            self.power_draw_base = DEFAULT_POWER_DRAW;
        }
        if (self.heat_output_base - 0.0).abs() < f64::EPSILON {
            self.heat_output_base = DEFAULT_HEAT_OUTPUT;
        }
        if self.purchase_cost == 0 {
            self.purchase_cost = DEFAULT_PURCHASE_COST;
        }
        self.last_reliability = self.reliability_base;
        self.last_heat = 0.0;
        self.last_effective_cooling = self.cooling_level;
        self.last_power_draw = self.idle_power_draw();
    }

    pub fn idle_power_draw(&self) -> f64 {
        let cooling_factor = 1.0 + ELECTRIC_COOLING_FACTOR * self.cooling_level as f64;
        (self.power_draw_base * cooling_factor).max(0.0)
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.status, ProcessorStatus::Idle)
    }

    pub fn is_functional(&self) -> bool {
        !matches!(
            self.status,
            ProcessorStatus::BurntOut | ProcessorStatus::Destroyed
        )
    }

    pub fn supports(&self, tag: &str) -> bool {
        self.instruction_set.iter().any(|known| known == tag)
    }

    pub fn assign(&mut self, job: Job, total_ms: u64, daemon_penalty: Option<DaemonPenalty>) {
        self.status = ProcessorStatus::Working(Box::new(ProcessorWork {
            job,
            remaining_ms: total_ms,
            total_ms,
            daemon_penalty,
            overheating: false,
        }));
        self.last_power_draw = self.idle_power_draw();
    }

    pub fn tick(
        &mut self,
        delta_ms: u64,
        rng: &mut impl Rng,
        cooling_bonus_levels: u8,
    ) -> Option<ProcessorEvent> {
        let evaluation_snapshot = match &self.status {
            ProcessorStatus::Working(work) => {
                Some(self.evaluate_job(&work.job, cooling_bonus_levels))
            }
            _ => None,
        };
        match &mut self.status {
            ProcessorStatus::Idle => {
                self.last_power_draw = self.idle_power_draw();
                None
            }
            ProcessorStatus::BurntOut | ProcessorStatus::Destroyed => None,
            ProcessorStatus::Working(work) => {
                let evaluation = evaluation_snapshot.expect("evaluation missing");
                self.last_reliability = evaluation.reliability;
                self.last_heat = evaluation.heat;
                self.last_effective_cooling = evaluation.effective_cooling;
                self.last_power_draw = evaluation.power_draw;

                if evaluation.reliability <= 0.0 || rng.gen_range(0.0..1.0) > evaluation.reliability
                {
                    let job = work.job.clone();
                    self.status = ProcessorStatus::BurntOut;
                    return Some(ProcessorEvent::BurntOut { job });
                }

                if self.finite_lifespan && self.mttf_ticks > 0 {
                    let base_wear = delta_ms as f64 / self.mttf_ticks as f64;
                    let heat_wear = evaluation.heat.max(0.0) * 0.0005 * (delta_ms as f64 / 1000.0);
                    let hazard_wear = evaluation.hazard_penalty * 0.05;
                    self.wear += base_wear + heat_wear + hazard_wear;
                    if self.wear >= 1.0 {
                        let job = work.job.clone();
                        self.status = ProcessorStatus::Destroyed;
                        return Some(ProcessorEvent::Destroyed { job });
                    }
                }

                if work.remaining_ms > delta_ms {
                    work.remaining_ms -= delta_ms;
                    work.overheating = evaluation.heat > 1.0
                        || self.requires_cooling_min > evaluation.effective_cooling;
                    None
                } else {
                    let completed_job = CompletedJob {
                        job: work.job.clone(),
                        daemon_penalty: work.daemon_penalty.clone(),
                    };
                    self.status = ProcessorStatus::Idle;
                    Some(ProcessorEvent::Completed(completed_job))
                }
            }
        }
    }

    pub fn remaining_and_total(&self) -> Option<(u64, u64)> {
        match &self.status {
            ProcessorStatus::Working(work) => Some((work.remaining_ms, work.total_ms)),
            _ => None,
        }
    }

    pub fn replace(&mut self) {
        self.status = ProcessorStatus::Idle;
        self.wear = 0.0;
        self.last_heat = 0.0;
        self.last_reliability = self.reliability_base;
        self.last_effective_cooling = self.cooling_level;
        self.last_power_draw = self.idle_power_draw();
    }

    pub fn reliability_display(&self) -> f64 {
        self.last_reliability.max(0.0)
    }

    pub fn heat_display(&self) -> f64 {
        self.last_heat
    }

    pub fn cooling_cap(&self) -> u8 {
        self.cooling_cap
    }

    pub fn last_power_draw(&self) -> f64 {
        self.last_power_draw
    }

    pub fn evaluate_job(&self, job: &Job, cooling_bonus_levels: u8) -> JobEvaluation {
        let effective_cooling =
            effective_cooling_level(self.cooling_level, self.cooling_cap, cooling_bonus_levels);
        let cooling_reduction = cooling_reduction(effective_cooling);
        let mut heat =
            self.heat_output_base * (1.0 + load_modifier(&self.power_draw_mod, &job.tag));
        heat *= 1.0 - cooling_reduction;
        if self.cooling_required && effective_cooling == 0 {
            heat += 1.2;
        }
        if self.requires_cooling_min > effective_cooling {
            heat += 0.8 * (self.requires_cooling_min - effective_cooling) as f64;
        }
        let hazard = tag_hazard(&job.tag);
        let hazard_penalty = hazard * hardening_multiplier(self.hardening_level, &job.tag);
        let mut reliability = self.reliability_base;
        reliability -= heat.max(0.0) * HEAT_FAILURE_MULTIPLIER;
        reliability -= hazard_penalty;
        reliability += cooling_reliability_bonus(effective_cooling);
        if self.cooling_required && effective_cooling == 0 {
            reliability -= 0.25;
        }
        if self.requires_cooling_min > effective_cooling {
            reliability -= 0.15 * (self.requires_cooling_min - effective_cooling) as f64;
        }
        reliability -= self.fragility * heat.max(0.0);
        reliability = reliability.clamp(0.0, 0.999);
        let cooling_factor = 1.0 + ELECTRIC_COOLING_FACTOR * effective_cooling as f64;
        let mut power_draw =
            self.power_draw_base * (1.0 + load_modifier(&self.power_draw_mod, &job.tag));
        if power_draw < 0.0 {
            power_draw = 0.0;
        }
        let power_draw = (power_draw * cooling_factor).max(0.0);
        JobEvaluation {
            reliability,
            heat,
            effective_cooling,
            hazard_penalty,
            power_draw,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JobEvaluation {
    pub reliability: f64,
    pub heat: f64,
    pub effective_cooling: u8,
    pub hazard_penalty: f64,
    pub power_draw: f64,
}

fn effective_cooling_level(level: u8, cap: u8, bonus: u8) -> u8 {
    let effective = level as u16 + bonus as u16;
    let max_allowed = cap as u16 + bonus as u16;
    effective.min(max_allowed) as u8
}

fn cooling_reduction(level: u8) -> f64 {
    match level {
        0 => 0.0,
        1 => 0.25,
        2 => 0.45,
        3 => 0.60,
        other => 0.60 + 0.05 * (other.saturating_sub(3)) as f64,
    }
}

fn cooling_reliability_bonus(level: u8) -> f64 {
    match level {
        0 => 0.0,
        1 => 0.01,
        2 => 0.02,
        3 => 0.03,
        other => 0.03 + 0.005 * (other.saturating_sub(3)) as f64,
    }
}

fn tag_hazard(tag: &str) -> f64 {
    match tag {
        "RADIATION" => 0.02,
        "ANGEL" => 0.03,
        "SURVEILLANCE" => 0.01,
        "SIMD" => 0.015,
        _ => 0.0,
    }
}

fn hardening_multiplier(level: u8, tag: &str) -> f64 {
    if tag == "RADIATION" || tag == "ANGEL" || tag == "SURVEILLANCE" {
        (1.0 - 0.2 * level as f64).max(0.2)
    } else {
        (1.0 - 0.05 * level as f64).max(0.5)
    }
}

fn load_modifier(mods: &HashMap<String, f64>, tag: &str) -> f64 {
    mods.get(tag).copied().unwrap_or(0.0)
}

#[derive(Debug)]
pub enum ProcessorEvent {
    Completed(CompletedJob),
    BurntOut { job: Job },
    Destroyed { job: Job },
}

#[derive(Debug, Clone)]
pub struct CompletedJob {
    pub job: Job,
    pub daemon_penalty: Option<DaemonPenalty>,
}

#[derive(Debug, Error)]
pub enum AssignmentError {
    #[error("invalid processor index")]
    InvalidProcessor,
    #[error("processor is busy")]
    ProcessorBusy,
    #[error("processor lacks instruction {0}")]
    IncompatibleInstruction(String),
    #[error("processor is not operational")]
    ProcessorInoperative,
}
