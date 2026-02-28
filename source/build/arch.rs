use crate::AppResult;

use anyhow::bail;

pub enum Arch {
	Amd64,
	Arm64,
}

impl Arch {
	pub fn from_cli_arg(value: &str) -> AppResult<Self> {
		let this = match value {
			"amd64" => Self::Amd64,
			"arm64" => Self::Arm64,
			_ => bail!("unsupported --arch value: {value}"),
		};
		Ok(this)
	}

	pub fn rustc(&self) -> &'static str {
		match self {
			Self::Amd64 => "x86_64-linux-android",
			Self::Arm64 => "aarch64-linux-android",
		}
	}

	pub fn aapt(&self) -> &'static str {
		match self {
			Self::Amd64 => "x86_64",
			Self::Arm64 => "arm64-v8a",
		}
	}
}
