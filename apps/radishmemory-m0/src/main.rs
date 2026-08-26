use std::process::ExitCode;

use radishmemory_core as _;
use radishmemory_sqlite as _;

fn main() -> ExitCode {
    match radishmemory_m0::run_embedded_suite() {
        Ok(run) => {
            println!(
                "{}",
                serde_json::to_string_pretty(run.report())
                    .expect("runner report is always valid JSON")
            );
            if run.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&radishmemory_m0::error_report(&error))
                    .expect("runner error report is always valid JSON")
            );
            ExitCode::FAILURE
        }
    }
}
