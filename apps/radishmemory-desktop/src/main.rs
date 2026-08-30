// These package dependencies are consumed by the sibling library target. Keep the binary
// target's workspace-level unused dependency lint aware of that intentional split.
use directories as _;
use getrandom as _;
use radishmemory_application as _;
use rfd as _;
use time as _;

fn main() -> eframe::Result {
    radishmemory_desktop::run()
}
