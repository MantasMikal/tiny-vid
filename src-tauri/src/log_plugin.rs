//! Tauri log plugin builder: colored output, local timezone, target stripping.

pub fn build_log_plugin() -> tauri_plugin_log::Builder {
    use tauri_plugin_log::fern::colors::{Color, ColoredLevelConfig};
    use time::macros::format_description;

    let colors = ColoredLevelConfig::default()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Cyan)
        .debug(Color::Magenta)
        .trace(Color::BrightBlack);

    let timezone = tauri_plugin_log::TimezoneStrategy::UseLocal;
    let time_fmt = format_description!("[hour]:[minute]:[second]");

    let mut builder = tauri_plugin_log::Builder::new()
        .timezone_strategy(timezone.clone())
        .format(move |out, message, record| {
            let now = timezone.get_now();
            let ts = now.format(&time_fmt).unwrap_or_else(|_| "??:??:??".into());
            let target = record
                .target()
                .strip_prefix("tiny_vid_tauri::")
                .or_else(|| record.target().strip_prefix("tiny_vid::"))
                .unwrap_or(record.target());
            out.finish(format_args!(
                "{ts}  {level:5}  {target:5}  {message}",
                ts = ts,
                level = colors.color(record.level()),
                target = target,
                message = message
            ))
        });

    #[cfg(debug_assertions)]
    {
        builder = builder
            .level(log::LevelFilter::Debug)
            .level_for("tiny_vid_tauri", log::LevelFilter::Trace);
    }
    #[cfg(not(debug_assertions))]
    {
        builder = builder.level(log::LevelFilter::Info);
    }
    builder
}
