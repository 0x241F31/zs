use std::path::PathBuf;

use tokio::process::Command;

use crate::utils::exec;
use crate::{AppResult, tools};

pub struct Opts {
	pub apk: PathBuf,
	pub keystore: PathBuf,
	pub build_tools: tools::AndroidBuildTools,
}

pub async fn run(opts: Opts) -> AppResult {
	let mut cmd = Command::new(opts.build_tools.apksigner());
	cmd.arg("sign");
	cmd.arg("--ks").arg(opts.keystore);
	cmd.arg("--ks-key-alias").arg("android");
	cmd.arg("--ks-pass").arg("pass:android");
	cmd.arg("--key-pass").arg("pass:android");
	cmd.arg("--in").arg(opts.apk);

	exec(cmd).await?;

	Ok(())
}
