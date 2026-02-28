use crate::AppResult;

use std::fs::File;
use std::path::Path;

use tokio::fs;
use zip::ZipArchive;

pub async fn unpack_archive(path: &Path, output: &Path) -> AppResult {
	let path = path.to_path_buf();

	fs::remove_dir_all(&output).await.ok();
	fs::create_dir_all(&output).await?;

	let output = output.to_path_buf();

	let unpack = || {
		let file = File::open(path.clone())?;
		let mut archive = ZipArchive::new(file)?;
		archive.extract(output)?;
		AppResult::Ok(path)
	};

	tokio::task::spawn_blocking(unpack).await??;

	Ok(())
}
