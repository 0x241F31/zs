use std::path::Path;
use std::process;

use anyhow::{Context, ensure};
use tokio::io::{self, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::process::Command;

use crate::AppResult;

pub async fn exec(cmd: Command) -> AppResult<process::Output> {
	let cwd = std::env::current_dir()?;
	exec_at(cwd.as_path(), cmd).await
}

pub async fn exec_at(cwd: impl AsRef<Path>, mut cmd: Command) -> AppResult<process::Output> {
	let cwd = cwd.as_ref();
	cmd.stdout(process::Stdio::piped());
	cmd.stderr(process::Stdio::piped());
	cmd.current_dir(cwd);

	let pretty = format_cmd(&cmd, cwd);
	tracing::debug!("{pretty}");

	let mut child = cmd.spawn()?;

	let mut child_stdout = child.stdout.take().context("missing child stdout pipe")?;
	let mut child_stderr = child.stderr.take().context("missing child stderr pipe")?;

	let stdout_future = async {
		let mut stdout = Vec::new();
		child_stdout.read_to_end(&mut stdout).await?;
		Ok::<Vec<u8>, io::Error>(stdout)
	};

	let stderr_future = async {
		let mut stderr = Vec::new();
		let mut owner_stderr = io::stderr();
		let mut buf = [0_u8; 8192];

		loop {
			let read = child_stderr.read(&mut buf).await?;
			if read == 0 {
				break;
			}
			owner_stderr.write_all(&buf[..read]).await?;
			stderr.extend_from_slice(&buf[..read]);
		}

		owner_stderr.flush().await?;
		Ok::<Vec<u8>, io::Error>(stderr)
	};

	let child_future = child.wait();

	let (stdout, stderr, status) = tokio::try_join!(stdout_future, stderr_future, child_future)?;
	let output = process::Output { status, stdout, stderr };

	ensure!(output.status.success(), "Process failed, {status}");

	Ok(output)
}

fn format_cmd(cmd: &Command, cwd: &Path) -> String {
	let cmd = cmd.as_std();

	let mut out = String::with_capacity(128);

	let program = str::from_utf8(cmd.get_program().as_encoded_bytes()).unwrap();
	out.push_str(&format_arg(program, cwd));

	let len = cmd.get_args().len();

	if len == 0 {
		return out;
	}

	out.push(' ');

	for (i, arg) in cmd.get_args().enumerate() {
		let arg = str::from_utf8(arg.as_encoded_bytes()).unwrap();
		out.push_str(format_arg(arg, cwd).as_str());
		(i != len - 1).then(|| out.push(' '));
	}

	return out;
}

fn format_arg(value: &str, cwd: &Path) -> String {
	let path = Path::new(value);

	if !path.is_absolute() {
		return value.to_string();
	}

	if let Ok(relative) = path.strip_prefix(cwd) {
		return relative.display().to_string();
	}

	return value.to_string();
}
