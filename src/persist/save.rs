use crate::sim::game::GameState;
use anyhow::Result;
use ron::ser::PrettyConfig;
use std::fs;

use super::SAVE_FILE;

pub fn save_game(state: &GameState) -> Result<()> {
    let pretty = PrettyConfig::new();
    let serialized = ron::ser::to_string_pretty(state, pretty)?;
    fs::write(SAVE_FILE, serialized)?;
    Ok(())
}
