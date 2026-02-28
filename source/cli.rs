use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail, ensure};
use lexopt::{Arg, Parser, ValueExt};

use zs::AppResult;
use zs::build::Arch;
use zs::fetch::Dependency;

pub enum Command {
	Build(BuildArgs),
	Sign(SignArgs),
	Deploy(DeployArgs),
	Fetch(FetchArgs),
	Setup(SetupCommand),
}

pub fn parse_args(cwd: &Path) -> AppResult<Command> {
	let mut parser = Parser::from_env();

	let cmd = match parser.next()? {
		Some(Arg::Value(value)) => value.string()?,
		Some(arg) => bail!(arg.unexpected()),
		None => bail!("missing command"),
	};

	match cmd.as_str() {
		"build" => parse_build_args(cwd, &mut parser),
		"sign" => parse_sign_args(cwd, &mut parser),
		"deploy" => parse_deploy_args(cwd, &mut parser),
		"fetch" => parse_fetch_args(cwd, &mut parser),
		"setup" => parse_setup_opts(cwd, &mut parser),
		_ => bail!("unsupported command: {cmd}"),
	}
}

pub struct BuildArgs {
	pub package: String,
	pub manifest: PathBuf,
	pub java_files: Vec<PathBuf>,
	pub jars: Vec<PathBuf>,
	pub resources: Option<PathBuf>,
	pub sysmodules: Option<PathBuf>,
	pub libs: Vec<PathBuf>,
	pub output: PathBuf,
	pub jdk: PathBuf,
	pub android_platform: PathBuf,
	pub android_build_tools: PathBuf,
	pub arch: Arch,
}

fn parse_build_args(cwd: &Path, p: &mut Parser) -> AppResult<Command> {
	let mut package = None;
	let mut manifest = None;
	let mut java_files = Vec::new();
	let mut jars = Vec::new();
	let mut resources = None;
	let mut sysmodules = None;
	let mut libs = Vec::new();
	let mut output = None;
	let mut jdk = None;
	let mut android_platform = None;
	let mut android_build_tools = None;
	let mut arch = None;

	while let Some(arg) = p.next()? {
		match arg {
			Arg::Long("package") => {
				let value = p.value()?;
				let value = value.string()?;
				package = value.into();
			}
			Arg::Long("manifest") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				manifest = value.into();
			}
			Arg::Long("java") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				java_files.push(value);
			}
			Arg::Long("jar") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				jars.push(value);
			}
			Arg::Long("resources") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				resources = value.into();
			}
			Arg::Long("sysmodules") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				sysmodules = value.into();
			}
			Arg::Long("output") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				output = value.into();
			}
			Arg::Long("lib") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				libs.push(value);
			}
			Arg::Long("jdk") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				jdk = value.into();
			}
			Arg::Long("android-platform") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				android_platform = value.into();
			}
			Arg::Long("android-build-tools") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				android_build_tools = value.into();
			}
			Arg::Long("arch") => {
				let value = p.value()?;
				let value = value.string()?;
				let value = Arch::from_cli_arg(&value)?;
				arch = value.into();
			}
			_ => bail!(arg.unexpected()),
		}
	}

	let package = package.context("missing --package")?;
	let manifest = manifest.context("missing --manifest")?;
	ensure!(java_files.len() > 0, "missing --java (repeatable)");
	let jdk = jdk.context("missing --jdk")?;
	let android_platform = android_platform.context("missing --android-platform")?;
	let android_build_tools = android_build_tools.context("missing --android-build-tools")?;
	let arch = arch.context("missing --arch")?;
	let output = output.unwrap_or_else(|| cwd.join(".zs/build"));

	let opts = BuildArgs {
		package,
		manifest,
		java_files,
		jars,
		resources,
		sysmodules,
		libs,
		output,
		jdk,
		android_platform,
		android_build_tools,
		arch,
	};
	Ok(Command::Build(opts))
}

// =========================================================================

pub struct SignArgs {
	pub apk: PathBuf,
	pub android_build_tools: PathBuf,
	pub keystore: PathBuf,
}

fn parse_sign_args(cwd: &Path, p: &mut Parser) -> AppResult<Command> {
	let mut apk = None;
	let mut android_build_tools = None;
	let mut keystore = None;

	while let Some(arg) = p.next()? {
		match arg {
			Arg::Long("android-build-tools") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				android_build_tools = value.into();
			}
			Arg::Long("keystore") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				keystore = value.into();
			}
			Arg::Value(value) => {
				ensure!(apk.is_none(), "unexpected extra positional argument");
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				apk = value.into();
			}
			_ => bail!(arg.unexpected()),
		}
	}

	let apk = apk.context("missing apk path after `sign`")?;
	let android_build_tools = android_build_tools.context("missing --android-build-tools")?;
	let keystore = keystore.context("missing --keystore")?;

	Ok(Command::Sign(SignArgs { apk, android_build_tools, keystore }))
}

// =========================================================================

pub struct DeployArgs {
	pub apk: PathBuf,
	pub package: String,
	pub launch: bool,
}

fn parse_deploy_args(cwd: &Path, p: &mut Parser) -> AppResult<Command> {
	let mut apk = None;
	let mut package = None;
	let mut launch = false;

	while let Some(arg) = p.next()? {
		match arg {
			Arg::Long("package") => {
				let value = p.value()?;
				let value = value.string()?;
				package = value.into();
			}
			Arg::Long("launch") => {
				launch = true;
			}
			Arg::Value(value) => {
				ensure!(apk.is_none(), "unexpected extra positional argument");
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				apk = value.into();
			}
			_ => bail!(arg.unexpected()),
		}
	}

	let apk = apk.context("missing apk path after `deploy`")?;
	let package = package.context("missing --package")?;

	Ok(Command::Deploy(DeployArgs { apk, package, launch }))
}

// =========================================================================

pub struct FetchArgs {
	pub dependencies: Vec<Dependency>,
	pub output: PathBuf,
	pub coursier: Option<PathBuf>,
}

fn parse_fetch_args(cwd: &Path, p: &mut Parser) -> AppResult<Command> {
	let mut dependencies = Vec::new();
	let mut output = None;
	let mut coursier = None;

	while let Some(arg) = p.next()? {
		match arg {
			Arg::Long("output") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				output = value.into();
			}
			Arg::Long("coursier") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				coursier = value.into();
			}
			Arg::Value(value) => {
				let value = value.string()?;
				let dep = value.parse()?;
				dependencies.push(dep);
			}
			_ => bail!(arg.unexpected()),
		}
	}

	ensure!(dependencies.len() > 0, "missing dependencies after `fetch`");
	let output = output.context("missing --output")?;

	Ok(Command::Fetch(FetchArgs { dependencies, output, coursier }))
}

// =========================================================================

pub enum SetupCommand {
	Keystore(SetupKeystoreArgs),
	Sysmodules(SetupSysmodulesArgs),
}

pub struct SetupKeystoreArgs {
	pub keytool: PathBuf,
	pub output: PathBuf,
}

pub struct SetupSysmodulesArgs {
	pub jdk: PathBuf,
	pub android_platform: PathBuf,
	pub workdir: PathBuf,
	pub output: PathBuf,
	pub clean: bool,
}

fn parse_setup_opts(cwd: &Path, p: &mut Parser) -> AppResult<Command> {
	let target = match p.next()? {
		Some(Arg::Value(value)) => value.string()?,
		Some(arg) => bail!(arg.unexpected()),
		None => bail!("missing setup target: expected `keystore` or `sysmodules`"),
	};

	match target.as_str() {
		"keystore" => parse_setup_keystore_opts(cwd, p),
		"sysmodules" => parse_setup_sysmodules_opts(cwd, p),
		value => bail!("unsupported setup target: {value}"),
	}
}

fn parse_setup_keystore_opts(cwd: &Path, p: &mut Parser) -> AppResult<Command> {
	let mut keytool = None;
	let mut output = None;

	while let Some(arg) = p.next()? {
		match arg {
			Arg::Long("keytool") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				keytool = value.into();
			}
			Arg::Long("output") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				output = value.into();
			}
			_ => bail!(arg.unexpected()),
		}
	}

	let keytool = keytool.context("missing --keytool")?;
	let output = output.context("missing --output")?;

	let cmd = SetupCommand::Keystore(SetupKeystoreArgs { keytool, output });

	Ok(Command::Setup(cmd))
}

fn parse_setup_sysmodules_opts(cwd: &Path, p: &mut Parser) -> AppResult<Command> {
	let mut jdk = None;
	let mut android_platform = None;
	let mut workdir = None;
	let mut output = None;
	let mut clean = true;

	while let Some(arg) = p.next()? {
		match arg {
			Arg::Long("jdk") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				jdk = value.into();
			}
			Arg::Long("android-platform") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				android_platform = value.into();
			}
			Arg::Long("output") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				output = value.into();
			}
			Arg::Long("workdir") => {
				let value = p.value()?;
				let value = PathBuf::from(value);
				let value = resolve(cwd, value);
				workdir = value.into();
			}
			Arg::Long("no-clean") => {
				clean = false;
			}
			_ => bail!(arg.unexpected()),
		}
	}

	let jdk = jdk.context("missing --jdk")?;
	let android_platform = android_platform.context("missing --android-platform")?;
	let workdir = workdir.context("missing --workdir")?;
	let output = output.context("missing --output")?;

	let opts = SetupSysmodulesArgs { jdk, android_platform, workdir, output, clean };
	let cmd = SetupCommand::Sysmodules(opts);

	Ok(Command::Setup(cmd))
}

fn resolve(cwd: &Path, path: PathBuf) -> PathBuf {
	if path.is_absolute() {
		return path;
	}
	cwd.join(path)
}
