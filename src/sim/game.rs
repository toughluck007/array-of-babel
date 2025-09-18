use super::data_storage::DataStorage;
use super::economy;
use super::jobs::{self, Job};
use super::processors::{AssignmentError, CompletedJob, ProcessorState};
use rand::Rng;
use rand::rngs::ThreadRng;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;
use thiserror::Error;

const MAX_JOBS: usize = 5;
const MAX_MESSAGES: usize = 8;
const JOB_SPAWN_INTERVAL: Duration = Duration::from_secs(6);
const DAY_DURATION: Duration = Duration::from_secs(18);
pub const DAEMON_UNLOCK_CREDITS: u64 = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub credits: u64,
    pub processors: Vec<ProcessorState>,
    pub jobs: Vec<Job>,
    pub storage: DataStorage,
    pub daemon_unlocked: bool,
    pub daemon_enabled: bool,
    pub job_counter: u64,
    #[serde(default = "default_unlocked_tags")]
    pub unlocked_tags: Vec<String>,
    #[serde(default = "default_store_purchases")]
    pub store_purchases: Vec<u32>,
}

fn default_store_purchases() -> Vec<u32> {
    vec![0; STORE_ITEMS.len()]
}

fn default_unlocked_tags() -> Vec<String> {
    vec![jobs::GENERAL_TAG.to_string()]
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            credits: 120,
            processors: vec![ProcessorState::starter()],
            jobs: Vec::new(),
            storage: DataStorage::new(120),
            daemon_unlocked: false,
            daemon_enabled: false,
            job_counter: 0,
            unlocked_tags: default_unlocked_tags(),
            store_purchases: default_store_purchases(),
        }
    }
}

pub struct Game {
    pub state: GameState,
    job_spawn_timer: Duration,
    day_timer: Duration,
    rng: ThreadRng,
    messages: VecDeque<String>,
}

impl Game {
    pub fn fresh() -> Self {
        Self::from_state(GameState::default())
    }

    pub fn from_state(mut state: GameState) -> Self {
        if state.store_purchases.len() < STORE_ITEMS.len() {
            state.store_purchases.resize(STORE_ITEMS.len(), 0);
        }
        if state.unlocked_tags.is_empty() {
            state.unlocked_tags = default_unlocked_tags();
        }
        if !state
            .unlocked_tags
            .iter()
            .any(|tag| tag == jobs::GENERAL_TAG)
        {
            state.unlocked_tags.insert(0, jobs::GENERAL_TAG.to_string());
        }
        for processor in &mut state.processors {
            for tag in &state.unlocked_tags {
                if !processor.supports(tag) {
                    processor.instruction_set.push(tag.clone());
                }
            }
        }
        Self {
            state,
            job_spawn_timer: Duration::default(),
            day_timer: Duration::default(),
            rng: thread_rng(),
            messages: VecDeque::with_capacity(MAX_MESSAGES),
        }
    }

    pub fn update(&mut self, delta: Duration) {
        self.job_spawn_timer += delta;
        while self.job_spawn_timer >= JOB_SPAWN_INTERVAL {
            self.job_spawn_timer -= JOB_SPAWN_INTERVAL;
            self.spawn_job_if_possible();
        }

        self.day_timer += delta;
        while self.day_timer >= DAY_DURATION {
            self.day_timer -= DAY_DURATION;
            self.apply_daily_cycle();
        }

        self.tick_processors(delta);
        if !self.state.daemon_unlocked && self.state.credits >= DAEMON_UNLOCK_CREDITS {
            self.state.daemon_unlocked = true;
            self.push_message("Daemon automation unlocked. Press D to toggle.".to_string());
        }

        if self.state.daemon_enabled {
            self.try_daemon_assignment();
        }
    }

    pub fn take_job(&mut self, index: usize) -> Option<Job> {
        if index < self.state.jobs.len() {
            Some(self.state.jobs.remove(index))
        } else {
            None
        }
    }

    pub fn return_job(&mut self, job: Job) {
        if self.state.jobs.len() >= MAX_JOBS {
            self.push_message("Job board full; discarded returned job.".to_string());
        } else {
            self.state.jobs.insert(0, job);
        }
    }

    pub fn assign_job_to_processor(
        &mut self,
        job: Job,
        processor_index: usize,
        daemon: bool,
    ) -> Result<(), AssignmentError> {
        if processor_index >= self.state.processors.len() {
            return Err(AssignmentError::InvalidProcessor);
        }
        let job_tag = job.tag.clone();
        let job_name = job.name.clone();
        let duration_ms;
        let processor_name;
        {
            let processor = &mut self.state.processors[processor_index];
            if !processor.is_idle() {
                return Err(AssignmentError::ProcessorBusy);
            }
            if !processor.supports(&job_tag) {
                return Err(AssignmentError::IncompatibleInstruction(job_tag));
            }
            duration_ms = economy::assignment_duration_ms(&job, processor, daemon);
            processor.assign(job, duration_ms, daemon);
            processor_name = processor.name.clone();
        }
        let seconds = duration_ms as f64 / 1000.0;
        if daemon {
            self.push_message(format!(
                "Daemon queued {job_name} on {processor_name} ({seconds:.1}s, -quality)",
            ));
        } else {
            self.push_message(format!(
                "Assigned {job_name} to {processor_name} ({seconds:.1}s)",
            ));
        }
        Ok(())
    }

    pub fn toggle_daemon(&mut self) -> bool {
        if self.state.daemon_unlocked {
            self.state.daemon_enabled = !self.state.daemon_enabled;
            if self.state.daemon_enabled {
                self.push_message("Daemon enabled: idle processors will self-assign.".to_string());
            } else {
                self.push_message("Daemon disabled.".to_string());
            }
            true
        } else {
            false
        }
    }

    pub fn job_spawn_progress(&self) -> f64 {
        (self.job_spawn_timer.as_secs_f64() / JOB_SPAWN_INTERVAL.as_secs_f64()).min(1.0)
    }

    pub fn day_progress(&self) -> f64 {
        (self.day_timer.as_secs_f64() / DAY_DURATION.as_secs_f64()).min(1.0)
    }

    pub fn messages(&self) -> impl Iterator<Item = &String> {
        self.messages.iter()
    }

    pub fn add_message<S: Into<String>>(&mut self, message: S) {
        self.push_message(message.into());
    }

    pub fn is_instruction_unlocked(&self, tag: &str) -> bool {
        self.state.unlocked_tags.iter().any(|known| known == tag)
    }

    pub fn store_items(&self) -> &'static [StoreItem] {
        &STORE_ITEMS
    }

    pub fn item_cost(&self, index: usize) -> Option<u64> {
        let item = STORE_ITEMS.get(index)?;
        let purchases = *self.state.store_purchases.get(index).unwrap_or(&0);
        if let Some(max) = item.max_purchases {
            if purchases >= max {
                return None;
            }
        }
        Some(item.base_cost + item.cost_step * purchases as u64)
    }

    pub fn store_purchases(&self, index: usize) -> Option<u32> {
        self.state.store_purchases.get(index).copied()
    }

    pub fn purchase_item(&mut self, index: usize) -> Result<(), PurchaseError> {
        let item = STORE_ITEMS.get(index).ok_or(PurchaseError::InvalidItem)?;
        let purchases = *self.state.store_purchases.get(index).unwrap_or(&0);
        if let Some(max) = item.max_purchases {
            if purchases >= max {
                return Err(PurchaseError::MaxedOut { item: item.name });
            }
        }
        if let StoreAction::UnlockInstructionSet { tag } = item.action {
            if self.is_instruction_unlocked(tag) {
                return Err(PurchaseError::InstructionAlreadyUnlocked {
                    tag: tag.to_string(),
                });
            }
        }
        let cost = item.base_cost + item.cost_step * purchases as u64;
        if self.state.credits < cost {
            return Err(PurchaseError::InsufficientCredits { cost });
        }

        self.state.credits -= cost;
        match item.action {
            StoreAction::IncreaseSpeed => {
                for processor in &mut self.state.processors {
                    processor.speed += 0.05;
                }
                self.push_message("Clock tuning applied: +0.05 speed to processors.".to_string());
            }
            StoreAction::ImproveQuality => {
                for processor in &mut self.state.processors {
                    processor.quality_bias += 1;
                }
                self.push_message("Calibration improved processor quality bias.".to_string());
            }
            StoreAction::ExpandStorage => {
                self.state.storage.expand(80);
                self.push_message(format!(
                    "Storage capacity expanded to {} units.",
                    self.state.storage.capacity
                ));
            }
            StoreAction::UnlockInstructionSet { tag } => {
                if self.unlock_instruction_tag(tag) {
                    self.push_message(format!(
                        "Microcode integrated: processors now accept {tag} workloads."
                    ));
                    self.push_message(
                        "Advanced job stream unlocked; watch for specialized contracts."
                            .to_string(),
                    );
                }
            }
        }
        if let Some(entry) = self.state.store_purchases.get_mut(index) {
            *entry += 1;
        }
        self.push_message(format!("Purchased {} (-{cost} cr)", item.name));
        Ok(())
    }

    pub fn total_upkeep(&self) -> u64 {
        economy::upkeep_total(&self.state.processors)
    }

    fn unlock_instruction_tag(&mut self, tag: &str) -> bool {
        if self.is_instruction_unlocked(tag) {
            return false;
        }
        self.state.unlocked_tags.push(tag.to_string());
        for processor in &mut self.state.processors {
            if !processor.supports(tag) {
                processor.instruction_set.push(tag.to_string());
            }
        }
        true
    }

    fn choose_job_tag<'a>(&'a mut self) -> &'a str {
        let mut pool: Vec<&str> = Vec::new();
        for tag in &self.state.unlocked_tags {
            if !self
                .state
                .processors
                .iter()
                .any(|processor| processor.supports(tag.as_str()))
            {
                continue;
            }
            let weight = if tag == jobs::GENERAL_TAG { 4 } else { 2 };
            for _ in 0..weight {
                pool.push(tag.as_str());
            }
        }
        if pool.is_empty() {
            jobs::GENERAL_TAG
        } else {
            let idx = self.rng.gen_range(0..pool.len());
            pool[idx]
        }
    }

    fn spawn_job_if_possible(&mut self) {
        if self.state.jobs.len() >= MAX_JOBS {
            return;
        }
        self.state.job_counter += 1;
        let tag = self.choose_job_tag().to_string();
        let job = jobs::generate_job_with_tag(self.state.job_counter, &tag, &mut self.rng);
        let job_name = job.name.clone();
        self.state.jobs.push(job);
        self.push_message(format!("New job posted: {job_name} [{tag}]"));
    }

    fn tick_processors(&mut self, delta: Duration) {
        if delta.is_zero() {
            return;
        }
        let delta_ms = delta.as_millis() as u64;
        let mut completed = Vec::new();
        for (index, processor) in self.state.processors.iter_mut().enumerate() {
            if let Some(done) = processor.tick(delta_ms) {
                completed.push((index, done));
            }
        }
        for (index, done) in completed {
            self.resolve_completed_job(index, done);
        }
    }

    fn resolve_completed_job(&mut self, processor_index: usize, completed: CompletedJob) {
        if processor_index >= self.state.processors.len() {
            return;
        }
        let (quality, processor_name) = {
            let processor = &self.state.processors[processor_index];
            let processor_name = processor.name.clone();
            let quality = economy::roll_quality(
                &completed.job,
                processor,
                completed.daemon_penalty,
                &mut self.rng,
            );
            (quality, processor_name)
        };
        let payout = economy::payout_for_quality(&completed.job, quality);
        self.state.credits += payout;
        let stored = self.state.storage.store(completed.job.data_output);
        if stored < completed.job.data_output {
            let lost = completed.job.data_output - stored;
            if lost > 0 {
                self.push_message(format!(
                    "Storage overflow: {lost} data units released back into the ether."
                ));
            }
        }
        self.push_message(format!(
            "{} completed on {processor_name} | quality {quality} | +{payout} cr",
            completed.job.name
        ));
    }

    fn apply_daily_cycle(&mut self) {
        let upkeep = self.total_upkeep();
        if upkeep > 0 {
            if self.state.credits >= upkeep {
                self.state.credits -= upkeep;
                self.push_message(format!("Paid upkeep of {upkeep} credits."));
            } else {
                self.state.credits = 0;
                self.push_message(format!(
                    "Upkeep cost {upkeep} exceeded reserves; treasury depleted."
                ));
            }
        }
        let passive = economy::passive_income(self.state.storage.stored);
        if passive > 0 {
            self.state.credits += passive;
            self.push_message(format!("Passive data dividend +{passive} credits."));
        }
    }

    fn try_daemon_assignment(&mut self) {
        loop {
            if self.state.jobs.is_empty() {
                break;
            }
            let Some(processor_index) = self.state.processors.iter().position(|p| p.is_idle())
            else {
                break;
            };
            let job = self.state.jobs.remove(0);
            if let Err(err) = self.assign_job_to_processor(job, processor_index, true) {
                self.push_message(format!("Daemon failed assignment: {err}"));
                break;
            }
        }
    }

    fn push_message(&mut self, message: String) {
        if self.messages.len() >= MAX_MESSAGES {
            self.messages.pop_front();
        }
        self.messages.push_back(message);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StoreItem {
    pub name: &'static str,
    pub description: &'static str,
    pub base_cost: u64,
    pub cost_step: u64,
    pub action: StoreAction,
    pub max_purchases: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub enum StoreAction {
    IncreaseSpeed,
    ImproveQuality,
    ExpandStorage,
    UnlockInstructionSet { tag: &'static str },
}

const STORE_ITEMS: [StoreItem; 4] = [
    StoreItem {
        name: "Clock Tuning",
        description: "Trim execution cycles for all processors (+0.05 speed each purchase).",
        base_cost: 120,
        cost_step: 45,
        action: StoreAction::IncreaseSpeed,
        max_purchases: None,
    },
    StoreItem {
        name: "Precision Calibration",
        description: "Improve processor quality bias (+1 each purchase).",
        base_cost: 140,
        cost_step: 60,
        action: StoreAction::ImproveQuality,
        max_purchases: None,
    },
    StoreItem {
        name: "Storage Array Expansion",
        description: "Increase data capacity by +80 units.",
        base_cost: 100,
        cost_step: 55,
        action: StoreAction::ExpandStorage,
        max_purchases: None,
    },
    StoreItem {
        name: "Instruction Microcode",
        description: "Install SIMD microcode; unlocks advanced job stream and adds support to processors.",
        base_cost: 260,
        cost_step: 0,
        action: StoreAction::UnlockInstructionSet {
            tag: jobs::SIMD_TAG,
        },
        max_purchases: Some(1),
    },
];

#[derive(Debug, Error)]
pub enum PurchaseError {
    #[error("not enough credits (requires {cost})")]
    InsufficientCredits { cost: u64 },
    #[error("unknown store item")]
    InvalidItem,
    #[error("{item} is sold out")]
    MaxedOut { item: &'static str },
    #[error("{tag} instruction set already unlocked")]
    InstructionAlreadyUnlocked { tag: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::jobs::SIMD_TAG;

    #[test]
    fn purchasing_microcode_unlocks_simd_tag() {
        let mut game = Game::fresh();
        game.state.credits = 1_000;
        let idx = STORE_ITEMS
            .iter()
            .position(|item| matches!(item.action, StoreAction::UnlockInstructionSet { .. }))
            .expect("microcode item present");
        let cost = game
            .item_cost(idx)
            .expect("microcode should be purchasable");

        assert!(!game.is_instruction_unlocked(SIMD_TAG));
        game.purchase_item(idx).expect("purchase should succeed");

        assert!(game.is_instruction_unlocked(SIMD_TAG));
        assert!(game.state.unlocked_tags.iter().any(|tag| tag == SIMD_TAG));
        assert!(
            game.state
                .processors
                .iter()
                .all(|processor| processor.supports(SIMD_TAG))
        );
        assert_eq!(game.store_purchases(idx), Some(1));
        assert_eq!(game.state.credits, 1_000 - cost);
        assert!(matches!(
            game.purchase_item(idx),
            Err(PurchaseError::MaxedOut { .. })
        ));
    }
}
