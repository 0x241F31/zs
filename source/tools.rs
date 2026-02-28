use std::path::PathBuf;

pub struct Jdk {
	root: PathBuf,
}

impl Jdk {
	pub fn new(at: PathBuf) -> Self {
		Self { root: at }
	}

	pub fn root(&self) -> &PathBuf {
		&self.root
	}

	pub fn jar(&self) -> PathBuf {
		self.root.join("bin/jar")
	}

	pub fn javac(&self) -> PathBuf {
		self.root.join("bin/javac")
	}

	pub fn jvm(&self) -> PathBuf {
		self.root.join("bin/java")
	}

	pub fn jmod(&self) -> PathBuf {
		self.root.join("bin/jmod")
	}

	pub fn jlink(&self) -> PathBuf {
		self.root.join("bin/jlink")
	}
}

// =========================================================================

pub struct AndroidPlatform {
	root: PathBuf,
}

impl AndroidPlatform {
	pub fn new(at: PathBuf) -> Self {
		Self { root: at }
	}

	pub fn root(&self) -> &PathBuf {
		&self.root
	}

	pub fn android_jar(&self) -> PathBuf {
		self.root.join("android.jar")
	}

	pub fn platform_modules(&self) -> PathBuf {
		self.root.join("core-for-system-modules.jar")
	}
}

// =========================================================================

pub struct AndroidBuildTools {
	root: PathBuf,
}

impl AndroidBuildTools {
	pub fn new(at: PathBuf) -> Self {
		Self { root: at }
	}

	pub fn root(&self) -> &PathBuf {
		&self.root
	}

	pub fn aapt(&self) -> PathBuf {
		self.root.join("aapt")
	}

	pub fn apksigner(&self) -> PathBuf {
		self.root.join("apksigner")
	}

	pub fn d8_jar(&self) -> PathBuf {
		self.root.join("lib/d8.jar")
	}
}
