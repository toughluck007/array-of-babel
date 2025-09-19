use super::data_storage::DataStorage;
use super::economy;
use super::jobs::{self, Job};
use super::processors::{
    AssignmentError, CompletedJob, DaemonMode, JobEvaluation, ProcessorEvent, ProcessorState,
};
use rand::Rng;
use rand::rngs::ThreadRng;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::time::Duration;
use thiserror::Error;

const MAX_JOBS: usize = 5;
const MAX_MESSAGES: usize = 8;
const JOB_SPAWN_INTERVAL: Duration = Duration::from_secs(6);
const DAY_DURATION: Duration = Duration::from_secs(18);
pub const DAEMON_UNLOCK_CREDITS: u64 = 500;

#[derive(Debug, Clone)]
pub struct AssistSuggestion {
    pub job_index: usize,
    pub eta_secs: f64,
    pub reliability: f64,
    pub heat: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub credits: u64,
    pub processors: Vec<ProcessorState>,
    pub jobs: Vec<Job>,
    pub storage: DataStorage,
    pub daemon_unlocked: bool,
    pub daemon_enabled: bool,
    #[serde(default)]
    pub thermal_paste_timer_ms: u64,
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
            thermal_paste_timer_ms: 0,
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
        state.daemon_enabled = false;
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
            processor.ensure_runtime_defaults();
            if state.daemon_unlocked {
                processor.daemon_unlocked = true;
            }
            if state.daemon_enabled && processor.daemon_mode == DaemonMode::Off {
                processor.daemon_mode = DaemonMode::Auto;
            }
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

        if self.state.thermal_paste_timer_ms > 0 {
            let delta_ms = delta.as_millis() as u64;
            if delta_ms > 0 {
                if delta_ms >= self.state.thermal_paste_timer_ms {
                    self.state.thermal_paste_timer_ms = 0;
                    self.push_message("Thermal paste bonus has dissipated.".to_string());
                } else {
                    self.state.thermal_paste_timer_ms -= delta_ms;
                }
            }
        }

        if !self.state.daemon_unlocked && self.state.credits >= DAEMON_UNLOCK_CREDITS {
            self.state.daemon_unlocked = true;
            for processor in &mut self.state.processors {
                processor.daemon_unlocked = true;
            }
            self.push_message(
                "Daemon automation unlocked. Focus a processor and press D to cycle modes."
                    .to_string(),
            );
        }

        self.try_daemon_assignment();
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
            if !processor.is_functional() {
                return Err(AssignmentError::ProcessorInoperative);
            }
            let penalty = if daemon {
                Some(processor.daemon_penalty.clone())
            } else {
                None
            };
            duration_ms = economy::assignment_duration_ms(&job, processor, penalty.as_ref());
            processor.assign(job, duration_ms, penalty);
            processor_name = processor.name.clone();
        }
        let seconds = duration_ms as f64 / 1000.0;
        if daemon {
            self.push_message(format!(
                "Daemon queued {job_name} on {processor_name} ({seconds:.1}s, automation tax)",
            ));
        } else {
            self.push_message(format!(
                "Assigned {job_name} to {processor_name} ({seconds:.1}s)",
            ));
        }
        Ok(())
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

    pub fn item_cost(&self, index: usize, processor_index: Option<usize>) -> Option<u64> {
        let item = STORE_ITEMS.get(index)?;
        match item.action {
            StoreAction::ReplaceProcessor => {
                let processor = processor_index.and_then(|idx| self.state.processors.get(idx))?;
                let cost = replacement_cost_for_processor(processor);
                if cost == 0 { None } else { Some(cost) }
            }
            StoreAction::ReplaceModel => {
                let processor = processor_index.and_then(|idx| self.state.processors.get(idx))?;
                let cost = self.replacement_cost_for_model(&processor.name);
                if cost == 0 { None } else { Some(cost) }
            }
            StoreAction::UpgradeCooling => {
                let processor = processor_index.and_then(|idx| self.state.processors.get(idx))?;
                if processor.cooling_level >= processor.cooling_cap {
                    return None;
                }
                Some(item.base_cost + item.cost_step * processor.cooling_level as u64)
            }
            StoreAction::UpgradeHardening => {
                let processor = processor_index.and_then(|idx| self.state.processors.get(idx))?;
                if processor.hardening_level >= 3 {
                    return None;
                }
                Some(item.base_cost + item.cost_step * processor.hardening_level as u64)
            }
            StoreAction::InstallDaemonFirmware => {
                let processor = processor_index.and_then(|idx| self.state.processors.get(idx))?;
                if processor.daemon_unlocked {
                    return None;
                }
                Some(item.base_cost + item.cost_step * processor.daemon_priority.max(0) as u64)
            }
            _ => {
                let purchases = *self.state.store_purchases.get(index).unwrap_or(&0);
                if let Some(max) = item.max_purchases {
                    if purchases >= max {
                        return None;
                    }
                }
                Some(item.base_cost + item.cost_step * purchases as u64)
            }
        }
    }

    pub fn store_purchases(&self, index: usize) -> Option<u32> {
        self.state.store_purchases.get(index).copied()
    }

    pub fn purchase_item(
        &mut self,
        index: usize,
        processor_index: Option<usize>,
    ) -> Result<(), PurchaseError> {
        let item = STORE_ITEMS.get(index).ok_or(PurchaseError::InvalidItem)?;
        let purchases = *self.state.store_purchases.get(index).unwrap_or(&0);
        if let Some(max) = item.max_purchases {
            if purchases >= max {
                return Err(PurchaseError::MaxedOut { item: item.name });
            }
        }
        match item.action {
            StoreAction::ReplaceProcessor | StoreAction::ReplaceModel => {}
            _ => {
                if let StoreAction::UnlockInstructionSet { tag } = item.action {
                    if self.is_instruction_unlocked(tag) {
                        return Err(PurchaseError::InstructionAlreadyUnlocked {
                            tag: tag.to_string(),
                        });
                    }
                }
            }
        }
        let cost = match item.action {
            StoreAction::ReplaceProcessor => {
                let processor = processor_index
                    .and_then(|idx| self.state.processors.get(idx))
                    .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                let cost = replacement_cost_for_processor(processor);
                if cost == 0 {
                    return Err(PurchaseError::ProcessorHealthy);
                }
                cost
            }
            StoreAction::ReplaceModel => {
                let processor = processor_index
                    .and_then(|idx| self.state.processors.get(idx))
                    .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                let cost = self.replacement_cost_for_model(&processor.name);
                if cost == 0 {
                    return Err(PurchaseError::NoMatchingProcessors);
                }
                cost
            }
            StoreAction::UpgradeCooling => {
                let processor = processor_index
                    .and_then(|idx| self.state.processors.get(idx))
                    .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                if processor.cooling_level >= processor.cooling_cap {
                    return Err(PurchaseError::UpgradeAtCap);
                }
                item.base_cost + item.cost_step * processor.cooling_level as u64
            }
            StoreAction::UpgradeHardening => {
                let processor = processor_index
                    .and_then(|idx| self.state.processors.get(idx))
                    .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                if processor.hardening_level >= 3 {
                    return Err(PurchaseError::UpgradeAtCap);
                }
                item.base_cost + item.cost_step * processor.hardening_level as u64
            }
            StoreAction::InstallDaemonFirmware => {
                let processor = processor_index
                    .and_then(|idx| self.state.processors.get(idx))
                    .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                if processor.daemon_unlocked {
                    return Err(PurchaseError::DaemonAlreadyInstalled);
                }
                item.base_cost + item.cost_step * processor.daemon_priority.max(0) as u64
            }
            _ => item.base_cost + item.cost_step * purchases as u64,
        };

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
            StoreAction::UpgradeCooling => {
                let (name, level) = {
                    let processor = processor_index
                        .and_then(|idx| self.state.processors.get_mut(idx))
                        .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                    if processor.cooling_level >= processor.cooling_cap {
                        return Err(PurchaseError::UpgradeAtCap);
                    }
                    processor.cooling_level += 1;
                    processor.ensure_runtime_defaults();
                    (processor.name.clone(), processor.cooling_level)
                };
                self.push_message(format!("{name} cooling upgraded to level {level}."));
            }
            StoreAction::UpgradeHardening => {
                let (name, level) = {
                    let processor = processor_index
                        .and_then(|idx| self.state.processors.get_mut(idx))
                        .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                    if processor.hardening_level >= 3 {
                        return Err(PurchaseError::UpgradeAtCap);
                    }
                    processor.hardening_level += 1;
                    (processor.name.clone(), processor.hardening_level)
                };
                self.push_message(format!("{name} hardening increased to level {level}."));
            }
            StoreAction::ApplyThermalPaste => {
                self.state.thermal_paste_timer_ms = DAY_DURATION.as_millis() as u64;
                self.push_message(
                    "Thermal paste refreshed: cooling bonus active this cycle.".to_string(),
                );
            }
            StoreAction::InstallDaemonFirmware => {
                let name = {
                    let processor = processor_index
                        .and_then(|idx| self.state.processors.get_mut(idx))
                        .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                    processor.daemon_unlocked = true;
                    processor.daemon_penalty.quality = processor.daemon_penalty.quality.max(-3);
                    processor.daemon_penalty.time_multiplier =
                        (processor.daemon_penalty.time_multiplier - 0.02).max(1.02);
                    processor.name.clone()
                };
                self.push_message(format!(
                    "{name} daemon firmware installed. Automation penalties eased."
                ));
            }
            StoreAction::ReplaceProcessor => {
                let name = {
                    let processor = processor_index
                        .and_then(|idx| self.state.processors.get_mut(idx))
                        .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                    if processor.is_functional() {
                        return Err(PurchaseError::ProcessorHealthy);
                    }
                    processor.replace();
                    processor.name.clone()
                };
                self.push_message(format!(
                    "Replaced {name} chassis. Unit restored to service."
                ));
            }
            StoreAction::ReplaceModel => {
                let name = {
                    let processor = processor_index
                        .and_then(|idx| self.state.processors.get(idx))
                        .ok_or(PurchaseError::ProcessorSelectionRequired)?;
                    processor.name.clone()
                };
                let mut replaced = 0;
                for unit in &mut self.state.processors {
                    if unit.name == name && !unit.is_functional() {
                        unit.replace();
                        replaced += 1;
                    }
                }
                if replaced == 0 {
                    return Err(PurchaseError::NoMatchingProcessors);
                }
                self.push_message(format!(
                    "Replaced {replaced} units of {name}. Fleet restored.",
                ));
            }
        }
        if !matches!(
            item.action,
            StoreAction::ReplaceProcessor | StoreAction::ReplaceModel
        ) {
            if let Some(entry) = self.state.store_purchases.get_mut(index) {
                *entry += 1;
            }
        }
        self.push_message(format!("Purchased {} (-{cost} cr)", item.name));
        Ok(())
    }

    pub fn total_upkeep(&self) -> u64 {
        economy::upkeep_total(&self.state.processors)
    }

    pub fn total_electricity_cost(&self) -> u64 {
        economy::electricity_cost(&self.state.processors)
    }

    pub fn total_power_draw(&self) -> f64 {
        self.state
            .processors
            .iter()
            .map(|processor| processor.last_power_draw())
            .sum()
    }

    pub fn thermal_paste_active(&self) -> bool {
        self.state.thermal_paste_timer_ms > 0
    }

    pub fn accept_assist_suggestion(&mut self, processor_index: usize) -> bool {
        let processor_name = {
            let Some(processor) = self.state.processors.get(processor_index) else {
                self.push_message("Select a valid processor.".to_string());
                return false;
            };
            if !processor.daemon_unlocked || processor.daemon_mode != DaemonMode::Assist {
                self.push_message(format!(
                    "{} is not running Assist automation.",
                    processor.name
                ));
                return false;
            }
            if !processor.is_functional() {
                self.push_message(format!(
                    "{} is offline and cannot take suggestions.",
                    processor.name
                ));
                return false;
            }
            if !processor.is_idle() {
                self.push_message(format!("{} is already working.", processor.name));
                return false;
            }
            processor.name.clone()
        };

        let Some(suggestion) = self.assist_suggestion(processor_index) else {
            self.push_message(format!(
                "{processor_name} has no suggestions ready. Queue a job manually."
            ));
            return false;
        };

        if suggestion.job_index >= self.state.jobs.len() {
            self.push_message("Suggested job is no longer available.".to_string());
            return false;
        }

        let job = self.state.jobs.remove(suggestion.job_index);
        let job_clone = job.clone();
        match self.assign_job_to_processor(job_clone, processor_index, false) {
            Ok(()) => true,
            Err(err) => {
                let reinsertion = suggestion.job_index.min(self.state.jobs.len());
                self.state.jobs.insert(reinsertion, job);
                self.push_message(format!("Assist assignment failed: {err}"));
                false
            }
        }
    }

    fn replacement_cost_for_model(&self, name: &str) -> u64 {
        self.state
            .processors
            .iter()
            .filter(|processor| processor.name == name && !processor.is_functional())
            .map(replacement_cost_for_processor)
            .sum()
    }

    fn store_index_for(action: StoreAction) -> Option<usize> {
        STORE_ITEMS.iter().position(|item| item.action == action)
    }

    pub fn replace_processor_direct(&mut self, index: usize) -> Result<(), PurchaseError> {
        let store_index = Self::store_index_for(StoreAction::ReplaceProcessor)
            .ok_or(PurchaseError::InvalidItem)?;
        let processor_index = Some(index);
        self.purchase_item(store_index, processor_index)
    }

    pub fn replace_model_direct(&mut self, index: usize) -> Result<(), PurchaseError> {
        let store_index =
            Self::store_index_for(StoreAction::ReplaceModel).ok_or(PurchaseError::InvalidItem)?;
        let processor_index = Some(index);
        self.purchase_item(store_index, processor_index)
    }

    pub fn cycle_daemon_mode(&mut self, index: usize) {
        let message = if let Some(processor) = self.state.processors.get_mut(index) {
            if !self.state.daemon_unlocked || !processor.daemon_unlocked {
                Some(format!(
                    "{} lacks daemon firmware. Install microcode to unlock.",
                    processor.name
                ))
            } else if !processor.is_functional() {
                Some(format!(
                    "{} is offline and cannot change automation mode.",
                    processor.name
                ))
            } else {
                processor.daemon_mode = match processor.daemon_mode {
                    DaemonMode::Off => DaemonMode::Assist,
                    DaemonMode::Assist => DaemonMode::Auto,
                    DaemonMode::Auto => DaemonMode::Off,
                };
                let label = match processor.daemon_mode {
                    DaemonMode::Off => "Off",
                    DaemonMode::Assist => "Assist",
                    DaemonMode::Auto => "Auto",
                };
                Some(format!("{} automation mode -> {label}.", processor.name))
            }
        } else {
            Some("Select a valid processor.".to_string())
        };
        if let Some(msg) = message {
            self.push_message(msg);
        }
    }

    pub fn toggle_honor_cooling(&mut self, index: usize) {
        let message = if let Some(processor) = self.state.processors.get_mut(index) {
            processor.honor_cooling_mins = !processor.honor_cooling_mins;
            let state = if processor.honor_cooling_mins {
                "will honor cooling minimums"
            } else {
                "will override cooling minimums"
            };
            Some(format!("{} {} when auto-assigning.", processor.name, state))
        } else {
            Some("Select a valid processor.".to_string())
        };
        if let Some(msg) = message {
            self.push_message(msg);
        }
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
        let cooling_bonus = if self.state.thermal_paste_timer_ms > 0 {
            1
        } else {
            0
        };
        let mut events = Vec::new();
        for (index, processor) in self.state.processors.iter_mut().enumerate() {
            if let Some(event) = processor.tick(delta_ms, &mut self.rng, cooling_bonus) {
                events.push((index, event));
            }
        }
        for (index, event) in events {
            match event {
                ProcessorEvent::Completed(done) => self.resolve_completed_job(index, done),
                ProcessorEvent::BurntOut { job } => self.handle_burnout(index, job),
                ProcessorEvent::Destroyed { job } => self.handle_destruction(index, job),
            }
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
                completed.daemon_penalty.as_ref(),
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

    fn handle_burnout(&mut self, processor_index: usize, job: Job) {
        if let Some(processor) = self.state.processors.get(processor_index) {
            let processor_name = processor.name.clone();
            self.push_message(format!(
                "{processor_name} burnt out while processing {}. Unit offline.",
                job.name
            ));
        }
    }

    fn handle_destruction(&mut self, processor_index: usize, job: Job) {
        if let Some(processor) = self.state.processors.get(processor_index) {
            let processor_name = processor.name.clone();
            self.push_message(format!(
                "{processor_name} was destroyed during {}. Replacement required.",
                job.name
            ));
        }
    }

    fn apply_daily_cycle(&mut self) {
        let upkeep = self.total_upkeep();
        let electricity = self.total_electricity_cost();
        let total_cost = upkeep + electricity;
        if total_cost > 0 {
            if self.state.credits >= total_cost {
                self.state.credits -= total_cost;
                if electricity > 0 {
                    self.push_message(format!(
                        "Paid upkeep {upkeep} cr + electricity {electricity} cr (total {total_cost})."
                    ));
                } else {
                    self.push_message(format!("Paid upkeep of {upkeep} credits."));
                }
            } else {
                self.state.credits = 0;
                self.push_message(format!(
                    "Operating costs {total_cost} exceeded reserves; treasury depleted."
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
        if self.state.jobs.is_empty() {
            return;
        }
        let cooling_bonus = if self.state.thermal_paste_timer_ms > 0 {
            1
        } else {
            0
        };
        let mut auto_indices: Vec<usize> = self
            .state
            .processors
            .iter()
            .enumerate()
            .filter(|(_, processor)| {
                processor.daemon_unlocked
                    && processor.daemon_mode == DaemonMode::Auto
                    && processor.is_idle()
                    && processor.is_functional()
            })
            .map(|(index, _)| index)
            .collect();

        auto_indices.sort_by(|a, b| {
            let pa = &self.state.processors[*a];
            let pb = &self.state.processors[*b];
            pb.daemon_priority
                .cmp(&pa.daemon_priority)
                .then_with(|| pb.speed.partial_cmp(&pa.speed).unwrap_or(Ordering::Equal))
        });

        for processor_index in auto_indices {
            if self.state.jobs.is_empty() {
                break;
            }
            let Some(job_index) = self.choose_daemon_job(processor_index, cooling_bonus) else {
                continue;
            };
            let job = self.state.jobs.remove(job_index);
            if let Err(err) = self.assign_job_to_processor(job, processor_index, true) {
                self.push_message(format!("Daemon failed assignment: {err}"));
            }
        }
    }

    fn choose_daemon_job(&self, processor_index: usize, cooling_bonus_levels: u8) -> Option<usize> {
        let processor = self.state.processors.get(processor_index)?;
        let mut best: Option<(usize, f64)> = None;
        for (job_index, job) in self.state.jobs.iter().enumerate() {
            if !processor.supports(&job.tag) {
                continue;
            }
            let evaluation = processor.evaluate_job(job, cooling_bonus_levels);
            if processor.honor_cooling_mins
                && processor.requires_cooling_min > evaluation.effective_cooling
                && job.tag != jobs::GENERAL_TAG
            {
                continue;
            }
            if evaluation.reliability < 0.35 {
                continue;
            }
            if processor.honor_cooling_mins && evaluation.heat > 1.8 {
                continue;
            }
            let duration =
                economy::assignment_duration_ms(job, processor, Some(&processor.daemon_penalty))
                    as f64;
            let base_score = if duration > 0.0 {
                (job.base_reward as f64 / duration).max(0.0)
            } else {
                job.base_reward as f64
            };
            let affinity = processor
                .daemon_affinity
                .get(&job.tag)
                .copied()
                .unwrap_or(0.0);
            let safety = (evaluation.reliability - 0.7) * 0.5;
            let score = base_score + affinity + safety;
            let update = match &best {
                Some((_, best_score)) => score > *best_score,
                None => true,
            };
            if update {
                best = Some((job_index, score));
            }
        }
        best.map(|(job_index, _)| job_index)
    }

    pub fn assist_suggestion(&self, index: usize) -> Option<AssistSuggestion> {
        let processor = self.state.processors.get(index)?;
        if !processor.daemon_unlocked
            || processor.daemon_mode != DaemonMode::Assist
            || !processor.is_idle()
            || !processor.is_functional()
        {
            return None;
        }
        if self.state.jobs.is_empty() {
            return None;
        }
        let cooling_bonus = if self.state.thermal_paste_timer_ms > 0 {
            1
        } else {
            0
        };
        let mut best: Option<(usize, f64, f64, JobEvaluation)> = None;
        for (job_index, job) in self.state.jobs.iter().enumerate() {
            if !processor.supports(&job.tag) {
                continue;
            }
            let evaluation = processor.evaluate_job(job, cooling_bonus);
            if evaluation.reliability < 0.3 {
                continue;
            }
            if processor.honor_cooling_mins
                && processor.requires_cooling_min > evaluation.effective_cooling
                && job.tag != jobs::GENERAL_TAG
            {
                continue;
            }
            let duration = economy::assignment_duration_ms(job, processor, None) as f64 / 1000.0;
            let score = if duration > 0.0 {
                (job.base_reward as f64 / duration).max(0.0)
            } else {
                job.base_reward as f64
            };
            let replace = match &best {
                Some((_, best_score, _, _)) => score > *best_score,
                None => true,
            };
            if replace {
                best = Some((job_index, score, duration, evaluation));
            }
        }
        best.map(|(job_index, _, duration, evaluation)| AssistSuggestion {
            job_index,
            eta_secs: duration,
            reliability: evaluation.reliability,
            heat: evaluation.heat,
        })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreAction {
    IncreaseSpeed,
    ImproveQuality,
    ExpandStorage,
    UnlockInstructionSet { tag: &'static str },
    UpgradeCooling,
    UpgradeHardening,
    ApplyThermalPaste,
    ReplaceProcessor,
    ReplaceModel,
    InstallDaemonFirmware,
}

const STORE_ITEMS: [StoreItem; 10] = [
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
    StoreItem {
        name: "Cooling Kit",
        description: "Install additional cooling on the selected processor (+1 level up to cap).",
        base_cost: 90,
        cost_step: 35,
        action: StoreAction::UpgradeCooling,
        max_purchases: None,
    },
    StoreItem {
        name: "Hardening Module",
        description: "Radiation shielding and error correction for the selected processor (+1 hardening).",
        base_cost: 140,
        cost_step: 55,
        action: StoreAction::UpgradeHardening,
        max_purchases: None,
    },
    StoreItem {
        name: "Service-Grade Thermal Paste",
        description: "Refreshes thermal interface material for the day (temporary +1 cooling level).",
        base_cost: 60,
        cost_step: 20,
        action: StoreAction::ApplyThermalPaste,
        max_purchases: None,
    },
    StoreItem {
        name: "Daemon Microcode",
        description: "Unlock automation firmware for the selected processor and ease penalties.",
        base_cost: 180,
        cost_step: 80,
        action: StoreAction::InstallDaemonFirmware,
        max_purchases: None,
    },
    StoreItem {
        name: "Replace Selected Unit",
        description: "Swap the highlighted processor chassis at the model's service rate.",
        base_cost: 0,
        cost_step: 0,
        action: StoreAction::ReplaceProcessor,
        max_purchases: None,
    },
    StoreItem {
        name: "Replace Model Fleet",
        description: "Replace all burnt or destroyed units of the selected model at bulk rate.",
        base_cost: 0,
        cost_step: 0,
        action: StoreAction::ReplaceModel,
        max_purchases: None,
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
    #[error("select a processor first")]
    ProcessorSelectionRequired,
    #[error("selected processor is operational")]
    ProcessorHealthy,
    #[error("no matching processors require replacement")]
    NoMatchingProcessors,
    #[error("upgrade already at maximum level")]
    UpgradeAtCap,
    #[error("daemon firmware already installed")]
    DaemonAlreadyInstalled,
}

fn replacement_cost_for_processor(processor: &ProcessorState) -> u64 {
    if processor.is_functional() {
        return 0;
    }
    let base = (processor.purchase_cost as f64 * processor.replace_cost_ratio).round() as u64;
    base.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::jobs::{GENERAL_TAG, Job, SIMD_TAG};
    use crate::sim::processors::{DaemonMode, ProcessorStatus};

    #[test]
    fn purchasing_microcode_unlocks_simd_tag() {
        let mut game = Game::fresh();
        game.state.credits = 1_000;
        let idx = STORE_ITEMS
            .iter()
            .position(|item| matches!(item.action, StoreAction::UnlockInstructionSet { .. }))
            .expect("microcode item present");
        let cost = game
            .item_cost(idx, None)
            .expect("microcode should be purchasable");

        assert!(!game.is_instruction_unlocked(SIMD_TAG));
        game.purchase_item(idx, None)
            .expect("purchase should succeed");

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
            game.purchase_item(idx, None),
            Err(PurchaseError::MaxedOut { .. })
        ));
    }

    #[test]
    fn replacing_burnt_out_processor_spends_credits() {
        let mut game = Game::fresh();
        game.state.credits = 500;
        let processor = &mut game.state.processors[0];
        processor.status = ProcessorStatus::BurntOut;
        let expected_cost =
            ((processor.purchase_cost as f64) * processor.replace_cost_ratio).round() as u64;

        game.replace_processor_direct(0)
            .expect("replacement should succeed");

        assert_eq!(game.state.credits, 500 - expected_cost);
        assert!(matches!(
            game.state.processors[0].status,
            ProcessorStatus::Idle
        ));
        assert!(game.state.processors[0].wear <= f64::EPSILON);
    }

    #[test]
    fn cycling_daemon_mode_traverses_states() {
        let mut game = Game::fresh();
        game.state.daemon_unlocked = true;
        let processor = &mut game.state.processors[0];
        processor.daemon_unlocked = true;

        assert_eq!(processor.daemon_mode, DaemonMode::Off);
        game.cycle_daemon_mode(0);
        assert_eq!(game.state.processors[0].daemon_mode, DaemonMode::Assist);
        game.cycle_daemon_mode(0);
        assert_eq!(game.state.processors[0].daemon_mode, DaemonMode::Auto);
        game.cycle_daemon_mode(0);
        assert_eq!(game.state.processors[0].daemon_mode, DaemonMode::Off);
    }

    #[test]
    fn cooling_upgrade_respects_cap() {
        let mut game = Game::fresh();
        game.state.credits = 1_000;
        let processor_index = 0;
        let cooling_idx = STORE_ITEMS
            .iter()
            .position(|item| item.action == StoreAction::UpgradeCooling)
            .expect("cooling kit present");

        game.purchase_item(cooling_idx, Some(processor_index))
            .expect("upgrade should succeed");
        assert_eq!(game.state.processors[processor_index].cooling_level, 1);

        // Bump to cap
        game.purchase_item(cooling_idx, Some(processor_index))
            .expect("second upgrade should succeed");
        game.purchase_item(cooling_idx, Some(processor_index))
            .expect("third upgrade should succeed");

        assert_eq!(game.state.processors[processor_index].cooling_level, 3);
        assert!(matches!(
            game.purchase_item(cooling_idx, Some(processor_index)),
            Err(PurchaseError::UpgradeAtCap)
        ));
    }

    #[test]
    fn assist_mode_assigns_suggested_job() {
        let mut game = Game::fresh();
        game.state.daemon_unlocked = true;
        let processor = &mut game.state.processors[0];
        processor.daemon_unlocked = true;
        processor.daemon_mode = DaemonMode::Assist;

        game.state.jobs.push(Job {
            id: 42,
            name: "Assist Contract".to_string(),
            tag: GENERAL_TAG.to_string(),
            base_time_ms: 5_000,
            base_reward: 150,
            quality_target: 60,
            data_output: 30,
        });

        assert!(game.accept_assist_suggestion(0));
        assert!(game.state.jobs.is_empty());
        assert!(matches!(
            game.state.processors[0].status,
            ProcessorStatus::Working(_)
        ));
    }
}
