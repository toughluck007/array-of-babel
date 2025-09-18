mod load;
mod save;

pub use load::load_game;
pub use save::save_game;

pub const SAVE_FILE: &str = "save.ron";
