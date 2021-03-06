use fern::colors::{Color, ColoredLevelConfig};
use fern::Dispatch;
use std::path::Path;
pub use log::{error, warn, info, debug, trace};

pub fn setup_logger<P: AsRef<Path>>(log_file_path: P, log_level: log::LevelFilter, name: String) {
    let colors = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::Blue)
        .trace(Color::White);

    let base_config = Dispatch::new().level(log_level)
        .level_for("tokio_util", log::LevelFilter::Warn)
        .level_for("mio", log::LevelFilter::Warn)
        .level_for("tarpc", log::LevelFilter::Warn);

    let stderr_config = Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{time}] {level:<5} |{client:<2}| {message} [{target}]",
                time = chrono::Local::now().format("%H:%M:%S:%3f"),
                client = name,
                target = record.target(),
                level = colors.color(record.level()),
                message = message,
            ))
        })
        .chain(std::io::stderr());

    let file_config = Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{time}] {level:<5} {message} [{target}]",
                time = chrono::Local::now().format("%H:%M:%S:%3f"),
                target = record.target(),
                level = record.level(),
                message = message,
            ))
        })
        .chain(fern::log_file(log_file_path).expect("Could not open log file path"));

    base_config
        .chain(stderr_config)
        .chain(file_config)
        .apply()
        .expect("Could not initialize logger");
}
