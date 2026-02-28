mod cli;

// =========================================================================

use crate::cli::{Command, parse_args};

use zs::tasks::{deploy, sign};
use zs::{AppResult, build, fetch, setup, tools, trace};

fn main() -> AppResult {
	trace::configure()?;

	let cwd = std::env::current_dir()?;
	let cmd = parse_args(&cwd)?;

	let tokio = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
	tokio.block_on(main_async(cmd))?;

	Ok(())
}

async fn main_async(cmd: Command) -> AppResult {
	match cmd {
		Command::Setup(cmd) => match cmd {
			cli::SetupCommand::Keystore(args) => {
				let opts = setup::keystore::Opts { keytool: args.keytool, output: args.output };
				setup::keystore::generate(opts).await
			}
			cli::SetupCommand::Sysmodules(args) => {
				let opts = setup::sysmodules::Opts {
					jdk: tools::Jdk::new(args.jdk),
					platform: tools::AndroidPlatform::new(args.android_platform),
					workdir: args.workdir,
					output: args.output,
					clean: args.clean,
				};
				setup::sysmodules::build(opts).await
			}
		},
		Command::Build(args) => {
			let opts = build::Opts {
				package: args.package,
				platform: args.android_platform,
				build_tools: args.android_build_tools,
				manifest: args.manifest,
				java: args.java_files,
				jars: args.jars,
				sysmodules: args.sysmodules,
				resources: args.resources,
				libs: args.libs,
				arch: args.arch,
				jdk: args.jdk,
				workdir: args.output,
			};
			build::run(opts).await
		}
		Command::Sign(args) => {
			let opts = sign::Opts {
				apk: args.apk,
				keystore: args.keystore,
				build_tools: tools::AndroidBuildTools::new(args.android_build_tools),
			};
			sign::run(opts).await
		}
		Command::Deploy(args) => {
			let opts = deploy::Opts { apk: args.apk, package: args.package, launch: args.launch };
			deploy::run(opts).await
		}
		Command::Fetch(args) => {
			let opts = fetch::Opts {
				output: args.output,
				dependencies: args.dependencies,
				coursier: args.coursier,
			};
			fetch::run(opts).await
		}
	}
}
