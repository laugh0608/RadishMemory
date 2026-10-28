mod backend;
mod controller;
mod error;
mod fonts;
mod logging;
mod paths;
mod picker;
mod profile;
mod runtime;
mod startup;
mod ui;
mod worker;

pub use controller::LibraryController;
pub use error::{ApplicationFailureSummary, DesktopError, DesktopErrorCode, DesktopErrorReason};
pub use paths::ApplicationPaths;
pub use picker::{NativeFilePicker, PickerOutcome};
pub use profile::{HOST_PROFILE_CONTRACT_ID, HostProfile, load_or_create_host_profile};
pub use runtime::{ProductionRuntime, ProductionRuntimeError};
pub use ui::RadishMemoryApp;

pub fn run() -> eframe::Result {
    let startup = startup::Startup::parse(std::env::args_os().skip(1))
        .map_err(|error| eframe::Error::AppCreation(Box::new(error)))?;
    // Suppress upstream diagnostics before native window creation as well as provider access.
    logging::install().map_err(|error| eframe::Error::AppCreation(Box::new(error)))?;
    let title = if startup.is_acceptance() {
        "RadishMemory — Synthetic acceptance"
    } else {
        "RadishMemory"
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id("io.github.laugh0608.RadishMemory")
            .with_inner_size([1080.0, 720.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        title,
        options,
        Box::new(move |creation_context| {
            fonts::install(&creation_context.egui_ctx);
            Ok(Box::new(RadishMemoryApp::bootstrap_with_startup(startup)))
        }),
    )
}
