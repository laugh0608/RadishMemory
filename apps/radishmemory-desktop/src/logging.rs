//! Process-wide, sink-free diagnostics policy installed before any provider access.
use std::sync::OnceLock;

struct Discard;
impl log::Log for Discard {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        false
    }
    fn log(&self, _: &log::Record<'_>) {}
    fn flush(&self) {}
}
static LOGGER: Discard = Discard;
static INSTALLED: OnceLock<bool> = OnceLock::new();

pub(crate) fn install() -> Result<(), crate::DesktopError> {
    let installed = *INSTALLED.get_or_init(|| {
        // Panic payloads from upstream may carry arbitrary paths or sensitive data.
        // The worker channel reports termination without formatting that payload.
        std::panic::set_hook(Box::new(|_| {}));
        let installed = log::set_logger(&LOGGER).is_ok();
        log::set_max_level(log::LevelFilter::Off);
        installed
    });
    if installed {
        Ok(())
    } else {
        Err(crate::DesktopError::without_source(
            crate::DesktopErrorCode::Runtime,
            crate::DesktopErrorReason::LoggingUnavailable,
            false,
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn suppression_is_exercised_in_an_isolated_process() {
        for mode in ["empty", "foreign"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "logging::tests::suppression_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("RADISHMEMORY_LOG_SUPPRESSION_CHILD", mode)
                .output()
                .unwrap();
            assert!(output.status.success());
            for bytes in [&output.stdout, &output.stderr] {
                assert!(!String::from_utf8_lossy(bytes).contains("synthetic-sensitive-payload"));
            }
        }
    }
    #[test]
    #[ignore = "isolated global logger/panic-hook child invoked by parent"]
    fn suppression_child() {
        let mode = std::env::var("RADISHMEMORY_LOG_SUPPRESSION_CHILD").unwrap();
        if mode == "foreign" {
            log::set_logger(&super::LOGGER).unwrap();
            assert_eq!(
                super::install().unwrap_err().reason(),
                crate::DesktopErrorReason::LoggingUnavailable
            );
            assert_eq!(
                crate::worker::Worker::start().err().unwrap().reason(),
                crate::DesktopErrorReason::LoggingUnavailable
            );
            return;
        }
        assert_eq!(mode, "empty");
        super::install().unwrap();
        super::install().unwrap();
        assert_eq!(log::max_level(), log::LevelFilter::Off);
        // Even an upstream increase cannot enable an output sink in our logger.
        log::set_max_level(log::LevelFilter::Trace);
        log::error!("synthetic-sensitive-payload");
        assert!(
            std::thread::spawn(|| panic!("synthetic-sensitive-payload"))
                .join()
                .is_err()
        );
    }
}
