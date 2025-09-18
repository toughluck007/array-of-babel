use crate::sim::jobs::Job;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Jobs,
    Processors,
}

impl Default for FocusTarget {
    fn default() -> Self {
        FocusTarget::Jobs
    }
}

#[derive(Debug, Default)]
pub struct App {
    focus: FocusTarget,
    pub selected_job: usize,
    pub selected_processor: usize,
    pub selected_store_item: usize,
    pub store_open: bool,
    pub pending_job: Option<Job>,
}

impl App {
    pub fn new() -> Self {
        Self {
            focus: FocusTarget::Jobs,
            selected_job: 0,
            selected_processor: 0,
            selected_store_item: 0,
            store_open: false,
            pending_job: None,
        }
    }

    pub fn focus(&self) -> FocusTarget {
        self.focus
    }

    pub fn set_focus(&mut self, focus: FocusTarget) {
        self.focus = focus;
    }

    pub fn next_focus(&mut self) {
        self.focus = match self.focus {
            FocusTarget::Jobs => FocusTarget::Processors,
            FocusTarget::Processors => FocusTarget::Jobs,
        };
    }

    pub fn toggle_store(&mut self) {
        self.store_open = !self.store_open;
        if self.store_open {
            self.selected_store_item = 0;
        }
    }

    pub fn clamp_job_selection(&mut self, len: usize) {
        if len == 0 {
            self.selected_job = 0;
        } else if self.selected_job >= len {
            self.selected_job = len - 1;
        }
    }

    pub fn clamp_processor_selection(&mut self, len: usize) {
        if len == 0 {
            self.selected_processor = 0;
        } else if self.selected_processor >= len {
            self.selected_processor = len - 1;
        }
    }

    pub fn clamp_store_selection(&mut self, len: usize) {
        if len == 0 {
            self.selected_store_item = 0;
        } else if self.selected_store_item >= len {
            self.selected_store_item = len - 1;
        }
    }
}
