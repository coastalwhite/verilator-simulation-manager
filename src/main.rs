use std::path::Path;

use verilator_simulation_manager::monitor::IterationMonitor;
use verilator_simulation_manager::FuzzServer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, exit_request_channel) = std::sync::mpsc::channel();

    ctrlc::set_handler(move || {
        tx.send(()).expect("Could not send signal on channel");
    })
    .expect("Error setting Control-C handler");

    let cmd = format!(
        "/home/johndoe/Downloads/software_verilator/software/obj_dir/VSECURE_PLATFORM_RI5CY_CW",
    );
    let cmd = &Path::new(&cmd);

    let mut fuzz_server = FuzzServer::new(cmd, exit_request_channel)?;
    fuzz_server.fuzz_loop::<IterationMonitor<1000>>(10, Some(10_000))?;
    fuzz_server.clean()?;

    Ok(())
}
