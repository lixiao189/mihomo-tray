use std::fs;

use anyhow::{Context, Result};
use log::LevelFilter;
#[cfg(debug_assertions)]
use log4rs::append::console::ConsoleAppender;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

use crate::paths;

const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024;
const LOG_FILE_COUNT: u32 = 3;

pub fn init() -> Result<()> {
    let log_dir = paths::logs_dir()?;
    fs::create_dir_all(&log_dir)
        .with_context(|| format!("create log dir {}", log_dir.display()))?;

    let log_path = log_dir.join("mihomo-tray.log");
    let roller_pattern = log_dir.join("mihomo-tray.{}.log");
    let roller = FixedWindowRoller::builder()
        .build(
            roller_pattern
                .to_str()
                .context("log roller pattern is not valid UTF-8")?,
            LOG_FILE_COUNT,
        )
        .context("create log roller")?;
    let policy = CompoundPolicy::new(
        Box::new(SizeTrigger::new(MAX_LOG_SIZE)),
        Box::new(roller),
    );

    let file = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} {l} {t} - {m}{n}",
        )))
        .build(&log_path, Box::new(policy))
        .with_context(|| format!("create file appender {}", log_path.display()))?;

    #[cfg(debug_assertions)]
    let builder = {
        let console = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new(
                "{d(%H:%M:%S)} {l} {t} - {m}{n}",
            )))
            .build();
        Config::builder()
            .appender(Appender::builder().build("file", Box::new(file)))
            .appender(Appender::builder().build("console", Box::new(console)))
    };
    #[cfg(not(debug_assertions))]
    let builder = Config::builder().appender(Appender::builder().build("file", Box::new(file)));

    #[cfg(debug_assertions)]
    let root = Root::builder()
        .appender("file")
        .appender("console")
        .build(LevelFilter::Info);
    #[cfg(not(debug_assertions))]
    let root = Root::builder().appender("file").build(LevelFilter::Info);

    let config = builder.build(root).context("build log4rs config")?;
    log4rs::init_config(config).context("init log4rs")?;
    log::info!("logging initialized -> {}", log_path.display());
    Ok(())
}
