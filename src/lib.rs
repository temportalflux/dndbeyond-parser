use anyhow::Result;
pub use log::Level;

pub fn init(name: &str, ignore: &[&'static str]) -> Result<()> {
	use simplelog::*;
	let log_path = {
		let mut path = std::env::current_dir()?;
		path.push(format!("{}.log", name));
		path
	};
	if let Some(parent) = log_path.parent() {
		std::fs::create_dir_all(parent)?;
	}
	let file = std::fs::OpenOptions::new()
		.create(true)
		.write(true)
		.truncate(true)
		.open(&log_path)?;
	let cfg = {
		let mut builder = ConfigBuilder::new();
		builder
			.set_max_level(log::LevelFilter::Error)
			.set_time_format_str("%Y.%m.%d-%H.%M.%S")
			// Pads the names of levels so that they line up in the log.
			// [ERROR]
			// [ WARN]
			// [ INFO]
			// [DEBUG]
			// [TRACE]
			.set_level_padding(LevelPadding::Left)
			// Thread IDs/Names are logged for ALL statements (that aren't on main)
			.set_thread_level(log::LevelFilter::Error)
			.set_thread_mode(ThreadLogMode::Names)
			.set_thread_padding(ThreadPadding::Left(5))
			// Target is always logged so that readers know what owner logged each line
			.set_target_level(log::LevelFilter::Error)
			.set_location_level(log::LevelFilter::Off);
		for str in ignore.iter() {
			builder.add_filter_ignore_str(str);
		}
		builder.build()
	};
	CombinedLogger::init(vec![
		TermLogger::new(
			LevelFilter::Trace,
			cfg.clone(),
			TerminalMode::Mixed,
			ColorChoice::Auto,
		),
		WriteLogger::new(LevelFilter::Trace, cfg.clone(), file),
	])
	.unwrap();
	log::info!("Writing log to {}", log_path.display());
	log::info!("Executing: {:?}", std::env::args().collect::<Vec<_>>());
	Ok(())
}
