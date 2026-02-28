mod unpack;

use self::unpack::unpack_archive;
use crate::{AppResult, utils};

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{bail, ensure};
use reqwest::Client;
use std::ops::Not;
use tokio::fs;
use tokio::process::Command;

#[rustfmt::skip]
const MAVEN_MIRRORS: &[&str] = &[
	"https://maven.google.com",
	"https://repo.maven.apache.org/maven2"
];

#[derive(Clone, Debug)]
pub struct Opts {
	pub dependencies: Vec<Dependency>,
	pub output: PathBuf,
	pub coursier: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct Dependency {
	pub group: String,
	pub artifact: String,
	pub version: String,
}

impl fmt::Display for Dependency {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}:{}:{}", self.group, self.artifact, self.version)
	}
}

impl FromStr for Dependency {
	type Err = anyhow::Error;

	fn from_str(input: &str) -> Result<Self, Self::Err> {
		let mut parts = input.splitn(4, ':');

		let (Some(group), Some(artifact), Some(version)) = (
			parts.next().filter(|v| v.is_empty().not()),
			parts.next().filter(|v| v.is_empty().not()),
			parts.next().filter(|v| v.is_empty().not()),
		) else {
			bail!("dependency must be group:artifact:version");
		};

		Ok(Self { group: group.into(), artifact: artifact.into(), version: version.into() })
	}
}

fn build_http_client() -> Client {
	let client = Client::builder();
	let client = client.connect_timeout(Duration::from_secs(3));
	let client = client.https_only(true);
	client.build().unwrap()
}

pub async fn run(opts: Opts) -> AppResult {
	let dependencies = match opts.coursier.as_ref() {
		Some(bin) => resolve_via_coursier(bin, &opts.dependencies, &opts.output).await?,
		None => opts.dependencies.clone(),
	};

	println!("Downloading dependencies:");
	let list = dependencies.iter().map(Dependency::to_string);
	let list = list.collect::<Vec<_>>().join("\n");
	println!("{}", list);

	let client = build_http_client();

	for dep in dependencies.iter() {
		let dir = opts.output.join(&dep.group).join(&dep.artifact).join(&dep.version);
		fs::create_dir_all(&dir).await?;

		download_pom(&client, &dir, dep).await?;

		let dep_path = download_dep(&client, &dir, dep).await?;

		if dep_path.extension().is_some_and(|ext| ext == "aar") {
			unpack_archive(&dep_path, &dir.join("source")).await?;
		}

		println!("fetched {}", dep);
	}

	Ok(())
}

async fn resolve_via_coursier(coursier: &Path, deps: &[Dependency], output: &Path) -> AppResult<Vec<Dependency>> {
	let mut cmd = Command::new(coursier);
	cmd.arg("resolve");
	cmd.arg("--cache").arg(output.join(".coursier"));

	MAVEN_MIRRORS.iter().for_each(|m| {
		cmd.arg("-r").arg(m);
	});

	deps.iter().for_each(|dep| {
		cmd.arg(dep.to_string());
	});

	let out = utils::exec(cmd).await?;
	let text = str::from_utf8(out.stdout.as_slice())?;

	let deps = text.lines();
	let deps = deps.map(str::trim);
	let deps = deps.map(Dependency::from_str);
	let deps = deps.filter_map(Result::ok);
	let deps = deps.collect::<Vec<_>>();

	ensure!(deps.len() > 0, "Coursier did not resolve dependencies");

	Ok(deps)
}

async fn download_pom(client: &Client, dir: &Path, dep: &Dependency) -> AppResult<PathBuf> {
	let file_name = format!("{}-{}.pom", dep.artifact, dep.version);
	let mut last_error = None;

	let attempts = MAVEN_MIRRORS.iter();
	let attempts = attempts.map(|mirror| (mirror, file_name.clone()));

	for (mirror, file) in attempts {
		let url = remote_url(mirror, dep, &file);

		let data = match download(client, &url).await {
			Ok(data) => data,
			Err(error) => {
				last_error = Some((url, error));
				continue;
			}
		};

		let path = dir.join(&file);
		fs::write(&path, data).await?;

		return Ok(path);
	}

	if let Some((url, error)) = last_error {
		tracing::error!("failed to download {}: {}", url, error);
	}

	bail!("pom not found for {}:{}:{}", dep.group, dep.artifact, dep.version);
}

fn remote_url(mirror: &str, dep: &Dependency, file: &str) -> String {
	format!("{}/{}/{}/{}/{}", mirror, dep.group.replace('.', "/"), dep.artifact, dep.version, file)
}

pub async fn download(client: &Client, url: &str) -> AppResult<Vec<u8>> {
	let response = client.get(url).send().await?;
	let response = response.error_for_status()?;
	let response = response.bytes().await?;
	Ok(response.to_vec())
}

// =========================================================================

async fn download_dep(client: &Client, dir: &Path, dep: &Dependency) -> AppResult<PathBuf> {
	#[rustfmt::skip]
	let candidates = [
		format!("{}-{}.aar", dep.artifact, dep.version), 
		format!("{}-{}.jar", dep.artifact, dep.version),
	];

	let attempts = MAVEN_MIRRORS.iter();
	let attempts = attempts.flat_map(|mirror| candidates.clone().map(|file| (mirror, file)));
	let mut last_error = None;

	for (mirror, file) in attempts {
		let url = remote_url(mirror, dep, &file);

		let data = match download(client, &url).await {
			Ok(bytes) => bytes,
			Err(err) => {
				last_error = Some((url, err));
				continue;
			}
		};

		let path = dir.join(&file);
		fs::write(&path, data).await?;

		return Ok(path);
	}

	if let Some((url, err)) = last_error {
		tracing::error!("failed to download {}: {}", url, err);
	}

	bail!("failed to download {} from all mirrors", dep);
}
