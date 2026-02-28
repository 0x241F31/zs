//! Builds a minimal Java runtime image used as the `javac --system` input when
//! compiling Android app Java sources.
//!
//! Pipeline:
//! 1. Generate synthetic `module-info.java` for `java.base` by exporting packages
//!    discovered in Android's `core-for-system-modules.jar`.
//! 2. Compile that descriptor into `module-info.class`.
//! 3. Patch `module-info.class` into a copy of `core-for-system-modules.jar`.
//! 4. Create `java.base.jmod` from the patched jar (required by `jlink`).
//! 5. Link a minimal runtime image containing only `java.base`; this directory is
//!    then passed to `javac --system`.
//!
//! This is necessary because Android's core jar is not directly consumable as a
//! complete module image for `javac --system`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use tokio::fs;
use tokio::process::Command;

use anyhow::anyhow;
use owo_colors::OwoColorize;

use crate::{AppResult, tools, utils};

pub struct Opts {
	pub jdk: tools::Jdk,
	pub platform: tools::AndroidPlatform,
	pub workdir: PathBuf,
	pub output: PathBuf,
	pub clean: bool,
}

impl Opts {
	fn module_info_java(&self) -> PathBuf {
		self.workdir.join("module-info.java")
	}

	fn module_info_class(&self) -> PathBuf {
		self.workdir.join("module-info.class")
	}

	fn module_info_jmod(&self) -> PathBuf {
		self.workdir.join("module-info.jmod")
	}

	fn patched_platform_modules(&self) -> PathBuf {
		self.workdir.join("core-for-system-modules.patched.jar")
	}
}

pub async fn build(opts: Opts) -> AppResult {
	fs::remove_dir_all(&opts.workdir).await.ok();
	fs::create_dir_all(&opts.workdir).await?;

	output_base_module(&opts).await?;
	compile_base_module(&opts).await?;

	patch_core_jar(&opts).await?;
	create_base_jmod(&opts).await?;

	link(&opts).await?;

	if opts.clean {
		fs::remove_dir_all(&opts.workdir).await.ok();
	}

	Ok(())
}

async fn output_base_module(opts: &Opts) -> AppResult {
	let core_for_system_modules = opts.platform.platform_modules();
	println!("{}", jdk_stage_card(1, 5, "Generate module-info.java"));
	println!("  from: {}", utils::rel(&core_for_system_modules).display());
	println!("  into: {}", utils::rel(opts.module_info_java()).display());

	// List jar entries so we can discover package names from `.class` paths.
	let mut cmd = Command::new(opts.jdk.jar());
	cmd.arg("--list").arg("--file").arg(core_for_system_modules);

	let listing = utils::exec(cmd).await?;
	let listing = String::from_utf8(listing.stdout)?;

	let mut core_pkgs = BTreeSet::new();

	for line in listing.lines() {
		if !line.ends_with(".class") {
			continue;
		}
		// Skip top-level classes with no package; module exports are package-based.
		let Some((dir, _)) = line.rsplit_once('/') else {
			continue;
		};
		core_pkgs.insert(dir.replace('/', "."));
	}

	// Emit an export for every discovered package so javac can resolve symbols from this module.
	let mut module_info = String::from("module java.base {\n");
	for pkg in core_pkgs.iter() {
		module_info.push_str("\texports ");
		module_info.push_str(pkg);
		module_info.push_str(";\n");
	}
	module_info.push_str("}\n");

	fs::write(opts.module_info_java(), module_info).await?;

	Ok(())
}

async fn compile_base_module(opts: &Opts) -> AppResult {
	println!("{}", jdk_stage_card(2, 5, "Compile module-info.java"));

	let mut cmd = Command::new(opts.jdk.javac());
	// Avoid host JDK modules; compile descriptor in isolation against the Android core jar.
	cmd.arg("--system=none");
	cmd.arg("--patch-module").arg(format!("java.base={}", opts.platform.platform_modules().display()));
	cmd.arg("-d").arg(&opts.workdir);
	cmd.arg(opts.module_info_java());

	utils::exec(cmd).await?;

	Ok(())
}

async fn patch_core_jar(opts: &Opts) -> AppResult {
	let core_jar = opts.patched_platform_modules();
	println!("{}", jdk_stage_card(3, 5, "Patch core.jar"));
	println!("  from: {}", utils::rel(opts.platform.platform_modules()).display());
	println!("  into: {}", utils::rel(&core_jar).display());
	fs::copy(opts.platform.platform_modules(), &core_jar).await?;

	let mut cmd = Command::new(opts.jdk.jar());
	cmd.arg("--update");
	cmd.arg("--file").arg(&core_jar);
	// Use a relative jar entry name (`module-info.class`) instead of embedding an absolute host path.
	cmd.arg("-C").arg(&opts.workdir);
	// This replaces/adds only `module-info.class`, leaving all existing classes untouched.
	cmd.arg(opts.module_info_class().file_name().unwrap());

	utils::exec(cmd).await?;

	Ok(())
}

async fn create_base_jmod(opts: &Opts) -> AppResult {
	println!("{}", jdk_stage_card(4, 5, "Create module-info.jmod"));
	println!("  into: {}", utils::rel(opts.module_info_jmod()).display());

	// Reuse the local JDK's module version so jlink sees a coherent module identity.
	let jdk_version = {
		let mut cmd = Command::new(opts.jdk.jmod());
		cmd.arg("--version");
		let version = utils::exec(cmd).await?;
		let version = String::from_utf8(version.stdout).unwrap();
		let version = version.trim();
		version.to_string()
	};

	let java_base_platform = {
		let mut cmd = Command::new(opts.jdk.jmod());
		cmd.arg("describe").arg(opts.jdk.root().join("jmods/java.base.jmod"));

		let desc = utils::exec(cmd).await?;

		let desc = String::from_utf8(desc.stdout)?;

		// Carry over target platform metadata from the host JDK's `java.base.jmod`.
		let target = desc.lines().find_map(|line| line.strip_prefix("platform "));
		let target = target.ok_or_else(|| anyhow!("no platform line in jmod describe output"))?;
		let target = target.trim();

		target.to_string()
	};

	let mut cmd = Command::new(opts.jdk.jmod());
	cmd.arg("create");
	// Package patched classes into a jmod so jlink can produce a runtime image from it.
	cmd.arg("--class-path").arg(opts.patched_platform_modules());
	cmd.arg("--module-version").arg(jdk_version);
	cmd.arg("--target-platform").arg(java_base_platform);
	cmd.arg(opts.module_info_jmod());

	utils::exec(cmd).await?;

	Ok(())
}

async fn link(opts: &Opts) -> AppResult {
	println!("{}", jdk_stage_card(5, 5, "Link java system image"));
	println!("  into: {}", utils::rel(&opts.output).display());

	fs::remove_dir_all(&opts.output).await.ok();

	let mut cmd = Command::new(opts.jdk.jlink());
	// Build a minimal image that exposes module metadata/layout expected by `javac --system`.
	cmd.arg("--module-path").arg(opts.module_info_jmod());
	cmd.arg("--add-modules").arg("java.base");
	cmd.arg("--disable-plugin").arg("system-modules");
	cmd.arg("--endian").arg("little");
	cmd.arg("--output").arg(&opts.output);
	utils::exec(cmd).await?;

	// `jrt-fs.jar` is needed for tooling that expects the JRT filesystem provider in the image.
	fs::copy(opts.jdk.root().join("lib/jrt-fs.jar"), opts.output.join("lib/jrt-fs.jar")).await?;

	Ok(())
}

fn jdk_stage_card(step: usize, total: usize, title: &str) -> String {
	let label = format!("{}", "JAVA-IMAGE".bold().truecolor(184, 219, 255));
	let title = format!("{}", title.bold().truecolor(226, 242, 255));
	let count = format!("{}", format!("{step}/{total}").bold().truecolor(197, 228, 255));
	format!("{label} {count} :: {title}")
}
