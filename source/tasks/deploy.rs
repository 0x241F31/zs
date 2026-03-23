use std::path::PathBuf;

use tokio::process::Command;

use crate::AppResult;
use crate::utils::exec;

pub struct Opts {
	pub apk: PathBuf,
	pub package: String,
	pub launch: bool,
	pub device: Option<String>,
}

pub async fn run(opts: Opts) -> AppResult {
	_ = uninstall(opts.device.as_ref(), &opts.package).await;
	install(opts.device.as_ref(), &opts.apk).await?;

	if opts.launch {
		launch(opts.device.as_ref(), &opts.package).await?;
	}

	Ok(())
}

async fn uninstall(device: Option<&String>, pkg: &str) -> AppResult {
	let mut cmd = Command::new("adb");

	device.map(|t| cmd.arg("-s").arg(t));
	cmd.arg("uninstall").arg(pkg);

	let out = exec(cmd).await?;
	out.status.success().then(|| tracing::info!("removed existing package"));

	Ok(())
}

async fn install(device: Option<&String>, apk: &PathBuf) -> AppResult {
	let mut cmd = Command::new("adb");

	device.map(|t| cmd.arg("-s").arg(t));
	cmd.arg("install").arg(apk);

	exec(cmd).await?;

	Ok(())
}

async fn launch(device: Option<&String>, pkg: &str) -> AppResult {
	let mut cmd = Command::new("adb");

	device.map(|t| cmd.arg("-s").arg(t));
	cmd.arg("shell").arg("am");
	cmd.arg("start").arg("-n").arg(format!("{pkg}/.MainActivity"));

	exec(cmd).await?;

	Ok(())
}
