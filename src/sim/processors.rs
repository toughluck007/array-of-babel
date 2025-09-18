use crate::sim::jobs::Job;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorState {
    pub name: String,
    pub speed: f64,
    pub quality_bias: i8,
    pub instruction_set: Vec<String>,
    pub upkeep_cost: u64,
    #[serde(default)]
    pub status: ProcessorStatus,
}

impl ProcessorState {
    pub fn starter() -> Self {
        Self {
            name: "Model F12-Scalar".to_string(),
            speed: 1.0,
            quality_bias: 0,
            instruction_set: vec!["GENERAL".to_string()],
            upkeep_cost: 8,
            status: ProcessorStatus::Idle,
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.status, ProcessorStatus::Idle)
    }

    pub fn supports(&self, tag: &str) -> bool {
        self.instruction_set.iter().any(|known| known == tag)
    }

    pub fn assign(&mut self, job: Job, total_ms: u64, daemon_penalty: bool) {
        self.status = ProcessorStatus::Busy {
            job,
            remaining_ms: total_ms,
            total_ms,
            daemon_penalty,
        };
    }

    pub fn tick(&mut self, delta_ms: u64) -> Option<CompletedJob> {
        match &mut self.status {
            ProcessorStatus::Idle => None,
            ProcessorStatus::Busy {
                remaining_ms,
                job,
                daemon_penalty,
                total_ms: _,
            } => {
                if *remaining_ms > delta_ms {
                    *remaining_ms -= delta_ms;
                    None
                } else {
                    let completed_job = CompletedJob {
                        job: job.clone(),
                        daemon_penalty: *daemon_penalty,
                    };
                    self.status = ProcessorStatus::Idle;
                    Some(completed_job)
                }
            }
        }
    }

    pub fn remaining_and_total(&self) -> Option<(u64, u64)> {
        match &self.status {
            ProcessorStatus::Idle => None,
            ProcessorStatus::Busy {
                remaining_ms,
                total_ms,
                ..
            } => Some((*remaining_ms, *total_ms)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessorStatus {
    Idle,
    Busy {
        job: Job,
        remaining_ms: u64,
        total_ms: u64,
        daemon_penalty: bool,
    },
}

impl Default for ProcessorStatus {
    fn default() -> Self {
        ProcessorStatus::Idle
    }
}

#[derive(Debug)]
pub struct CompletedJob {
    pub job: Job,
    pub daemon_penalty: bool,
}

#[derive(Debug, Error)]
pub enum AssignmentError {
    #[error("invalid processor index")]
    InvalidProcessor,
    #[error("processor is busy")]
    ProcessorBusy,
    #[error("processor lacks instruction {0}")]
    IncompatibleInstruction(String),
}
