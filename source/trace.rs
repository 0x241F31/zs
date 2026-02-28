use crate::AppResult;

use std::io::IsTerminal;

use tracing::Level;
use tracing_log::LogTracer;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;

pub fn configure() -> AppResult {
	LogTracer::builder().with_max_level(tracing_log::log::LevelFilter::Info).init()?;

	let app_filter = tracing_subscriber::filter::filter_fn(|metadata| {
		if metadata.target().starts_with(env!("CARGO_CRATE_NAME")) {
			return *metadata.level() <= Level::DEBUG;
		}
		if metadata.target().starts_with("hyper") {
			return *metadata.level() <= Level::WARN;
		}
		return *metadata.level() <= Level::DEBUG;
	});

	let stdout_layer = tracing_subscriber::fmt::layer()
		.with_writer(std::io::stdout)
		.with_ansi(std::io::stdout().is_terminal())
		.with_file(true)
		.without_time()
		.with_line_number(true)
		.with_target(false)
		.with_filter(app_filter);

	let subscriber = tracing_subscriber::registry().with(stdout_layer);
	tracing::subscriber::set_global_default(subscriber)?;

	Ok(())
}
