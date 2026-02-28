use tokio::process::Command;

use crate::{AppResult, utils};
use std::path::PathBuf;

pub struct Opts {
	pub keytool: PathBuf,
	pub output: PathBuf,
}

pub async fn generate(opts: Opts) -> AppResult {
	let mut cmd = Command::new(opts.keytool);

	cmd.arg("-genkey").arg("-v");
	cmd.arg("-keystore").arg(opts.output);
	cmd.arg("-storepass").arg("android");
	cmd.arg("-keypass").arg("android");
	cmd.arg("-alias").arg("android");
	cmd.arg("-keyalg").arg("RSA").arg("-keysize").arg("2048");
	cmd.arg("-validity").arg("10000");
	cmd.arg("-dname").arg("C=US, O=Android, CN=Android Debug");

	utils::exec(cmd).await?;

	Ok(())
}
