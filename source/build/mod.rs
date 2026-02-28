mod arch;

use std::ops::Not;
use std::path::PathBuf;

use tokio::fs;
use tokio::process::Command;

use anyhow::ensure;

use crate::utils::{exec, stage_card};
use crate::{AppResult, tools, utils};

pub use self::arch::Arch;

pub struct Opts {
	// Inputs from CLI/config.
	pub package: String,
	pub manifest: PathBuf,
	pub java: Vec<PathBuf>,
	pub jars: Vec<PathBuf>,
	pub sysmodules: Option<PathBuf>,
	pub resources: Option<PathBuf>,
	pub libs: Vec<PathBuf>,
	pub arch: Arch,

	// Tool locations.
	pub jdk: PathBuf,
	pub platform: PathBuf,
	pub build_tools: PathBuf,

	// Build output directory.
	pub workdir: PathBuf,
}

struct BuildContext {
	// Inputs.
	package: String,
	manifest: PathBuf,
	java: Vec<PathBuf>,
	jars: Vec<PathBuf>,
	sysmodules: Option<PathBuf>,
	resources: Option<PathBuf>,
	libs: Vec<PathBuf>,
	arch: Arch,

	// Resolved tools.
	jdk: tools::Jdk,
	platform: tools::AndroidPlatform,
	build_tools: tools::AndroidBuildTools,

	// Workspace.
	workdir: WorkDir,
}

struct WorkDir {
	root: PathBuf,
}

impl WorkDir {
	fn new(at: PathBuf) -> Self {
		Self { root: at }
	}

	fn root(&self) -> &PathBuf {
		&self.root
	}

	fn java(&self) -> PathBuf {
		self.root.join("java")
	}

	fn dex(&self) -> PathBuf {
		self.root.join("dex")
	}

	fn apk(&self) -> PathBuf {
		self.root.join("app.apk")
	}
}

pub async fn run(opts: Opts) -> AppResult {
	const TOTAL_STEPS: usize = 4;

	let ctx = BuildContext {
		jdk: tools::Jdk::new(opts.jdk),
		platform: tools::AndroidPlatform::new(opts.platform),
		build_tools: tools::AndroidBuildTools::new(opts.build_tools),
		package: opts.package,
		manifest: opts.manifest,
		java: opts.java,
		jars: opts.jars,
		sysmodules: opts.sysmodules,
		resources: opts.resources,
		libs: opts.libs,
		arch: opts.arch,
		workdir: WorkDir::new(opts.workdir),
	};

	fs::create_dir_all(ctx.workdir.root()).await?;
	tracing::debug!("created output directory at {}", utils::rel(ctx.workdir.root()).display());

	println!("{}", stage_card(1, TOTAL_STEPS, "Generate R.java"));
	generate_r_java(&ctx).await?;

	println!("{}", stage_card(2, TOTAL_STEPS, "Compile Java"));
	let classes = compile_java(&ctx).await?;

	println!("{}", stage_card(3, TOTAL_STEPS, "Compile DEX"));
	compile_dex(&ctx, classes).await?;

	println!("{}", stage_card(4, TOTAL_STEPS, "Package APK"));
	package_apk(&ctx).await?;

	Ok(())
}

async fn generate_r_java(ctx: &BuildContext) -> AppResult {
	fs::create_dir_all(ctx.workdir.java()).await?;

	let mut cmd = Command::new(ctx.build_tools.aapt());
	cmd.args(["package", "-m"]);
	cmd.arg("-M").arg(&ctx.manifest);
	cmd.arg("-I").arg(ctx.platform.android_jar());
	cmd.arg("-J").arg(ctx.workdir.java());
	ctx.resources.as_ref().map(|res| cmd.arg("-S").arg(res));
	cmd.arg("-v");

	exec(cmd).await?;

	Ok(())
}

async fn compile_java(ctx: &BuildContext) -> AppResult<Vec<PathBuf>> {
	fs::create_dir_all(ctx.workdir.java()).await?;

	for jar in ctx.jars.iter() {
		ensure!(jar.is_file(), "expected file for --jar: {}", jar.display());
	}

	let mut cmd = Command::new(ctx.jdk.javac());
	ctx.sysmodules.as_ref().map(|sysmodules| cmd.arg("--system").arg(sysmodules));
	let mut classpath = Vec::with_capacity(1 + ctx.jars.len());
	classpath.push(ctx.platform.android_jar());
	classpath.extend(ctx.jars.clone());
	let classpath = std::env::join_paths(classpath)?;
	cmd.arg("-classpath").arg(classpath);
	cmd.arg("-source").arg("17");
	cmd.arg("-target").arg("17");
	cmd.args(ctx.java.as_slice());
	let package = ctx.package.replace('.', "/");
	cmd.arg(ctx.workdir.java().join(package).join("R.java"));
	cmd.arg("-d").arg(ctx.workdir.java());
	cmd.arg("-encoding").arg("UTF-8");
	cmd.arg("-Xlint");
	cmd.arg("-deprecation");
	cmd.arg("-verbose");

	let out = exec(cmd).await?;
	let log = out.stderr.as_slice();
	let log = std::str::from_utf8(log)?;

	let files = log.lines();
	let files = files.filter_map(|line| line.strip_prefix("[wrote "));
	let files = files.filter_map(|rest| rest.strip_suffix(']'));
	let files = files.filter(|path| path.ends_with(".class"));
	let files = files.map(PathBuf::from).collect::<Vec<_>>();

	Ok(files)
}

async fn compile_dex(ctx: &BuildContext, classes: Vec<PathBuf>) -> AppResult {
	let dex_out = ctx.workdir.dex();
	fs::create_dir_all(&dex_out).await?;

	let mut cmd = Command::new(ctx.jdk.jvm());
	cmd.arg("-cp").arg(ctx.build_tools.d8_jar()).arg("com.android.tools.r8.D8");
	cmd.arg("--lib").arg(ctx.platform.android_jar());
	cmd.args(classes);
	cmd.args(&ctx.jars);
	cmd.arg("--release");
	cmd.arg("--output").arg(&dex_out);

	exec(cmd).await?;

	Ok(())
}

async fn package_apk(ctx: &BuildContext) -> AppResult {
	let lib = ctx.workdir.dex().join("lib").join(ctx.arch.aapt());
	fs::create_dir_all(&lib).await?;

	for native_file in ctx.libs.iter() {
		ensure!(native_file.is_dir().not(), "expected file, got directory: {}", lib.display());
		let path = lib.clone().join(native_file.file_name().unwrap());
		fs::copy(native_file, path).await?;
	}

	let mut cmd = Command::new(ctx.build_tools.aapt());
	cmd.args(["package", "-f"]);
	cmd.arg("--min-sdk-version").arg("21");
	cmd.arg("--target-sdk-version").arg("36");
	cmd.arg("--error-on-failed-insert");
	cmd.arg("--error-on-missing-config-entry");
	cmd.arg("--debug-mode");
	cmd.arg("-M").arg(&ctx.manifest);
	cmd.arg("-0").arg("dex");
	cmd.arg("-0").arg("so");
	ctx.resources.as_ref().map(|res| cmd.arg("-S").arg(res));
	cmd.arg("-I").arg(ctx.platform.android_jar());
	cmd.arg("-F").arg(ctx.workdir.apk());
	cmd.arg(ctx.workdir.java());
	cmd.arg(ctx.workdir.dex());

	exec(cmd).await?;

	Ok(())
}
