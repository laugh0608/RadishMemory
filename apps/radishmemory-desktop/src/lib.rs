mod controller;
mod error;
mod paths;
mod picker;
mod profile;
mod runtime;
mod ui;

pub use controller::LibraryController;
pub use error::{ApplicationFailureSummary, DesktopError, DesktopErrorCode, DesktopErrorReason};
pub use paths::ApplicationPaths;
pub use picker::{NativeFilePicker, PickerOutcome};
pub use profile::{HOST_PROFILE_CONTRACT_ID, HostProfile, load_or_create_host_profile};
pub use runtime::{ProductionRuntime, ProductionRuntimeError};
pub use ui::RadishMemoryApp;

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id("io.github.laugh0608.RadishMemory")
            .with_inner_size([1080.0, 720.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RadishMemory",
        options,
        Box::new(|_creation_context| Ok(Box::new(RadishMemoryApp::bootstrap()))),
    )
}
