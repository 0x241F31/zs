use std::path::PathBuf;

use tokio::process::Command;

use crate::AppResult;
use crate::utils::exec;

pub struct Opts {
	pub apk: PathBuf,
	pub package: String,
	pub launch: bool,
}

pub async fn run(opts: Opts) -> AppResult {
	_ = uninstall(&opts.package).await;
	install(&opts.apk).await?;

	if opts.launch {
		launch(&opts.package).await?;
	}

	Ok(())
}

async fn uninstall(pkg: &str) -> AppResult {
	let mut cmd = Command::new("adb");
	cmd.arg("uninstall").arg(pkg);

	let out = exec(cmd).await?;
	out.status.success().then(|| tracing::info!("removed existing package"));

	Ok(())
}

async fn install(apk: &PathBuf) -> AppResult {
	let mut cmd = Command::new("adb");
	cmd.arg("install").arg(apk);

	exec(cmd).await?;

	Ok(())
}

async fn launch(pkg: &str) -> AppResult {
	let mut cmd = Command::new("adb");

	cmd.arg("shell").arg("am");
	cmd.arg("start").arg("-n").arg(format!("{pkg}/.MainActivity"));

	exec(cmd).await?;

	Ok(())
}
