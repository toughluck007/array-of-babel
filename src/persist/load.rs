use crate::sim::game::GameState;
use anyhow::Result;
use std::fs;
use std::io::ErrorKind;

use super::SAVE_FILE;

pub fn load_game() -> Result<Option<GameState>> {
    match fs::read_to_string(SAVE_FILE) {
        Ok(content) => {
            let state = ron::from_str(&content)?;
            Ok(Some(state))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}
