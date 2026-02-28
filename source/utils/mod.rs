pub mod cmd;

// =========================================================================

pub use cmd::*;

// =========================================================================

use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

pub type AppResult<T = ()> = anyhow::Result<T>;

pub fn rel(path: impl AsRef<Path>) -> PathBuf {
	let path = path.as_ref();
	let cwd = std::env::current_dir().unwrap();
	path.strip_prefix(&cwd).unwrap_or(path).to_path_buf()
}

pub fn stage_card(step: usize, total: usize, title: &str) -> String {
	let build = format!("{}", "BUILD".bold().truecolor(255, 190, 120));
	let title = format!("{}", title.bold().truecolor(255, 245, 232));
	let count = format!("{}", format!("{step}/{total}").truecolor(255, 215, 168));
	format!("{build} {count} :: {title}")
}

pub fn done_card() -> String {
	format!("{}", "[DONE]".bold().truecolor(120, 236, 160))
}
