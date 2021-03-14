mod args;
use anyhow::{Context, Result};
use args::Args;
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::process::{Child, Command};
use std::thread;

fn main() -> Result<()> {
    let args = Args::new();
    println!("INFO: Assuming you have build the project in release mode, run the configurator beforehand and that the generated configurations are present in {:?}", args.config_files_path);
    println!("INFO: Press CTRL+C to stop diagnosis server and all clients");
    println!("INFO: All logs will appear in this terminal or alternatively per binary in the 'logs' folder");
    // let diagnosis_server_handle = spawn_diagnosis_server()?;
    // let client_handles = spawn_clients()?;

    let mut subscribed_signals = Signals::new(&[SIGINT])?;
    thread::spawn(move || {
        for signal in subscribed_signals.forever() {
            match signal {
                SIGINT => {
                    println!(
                        "Received SIGINT signal ({}), shutting down clients and diagnosis server..",
                        signal
                    );
                }
                _ => unreachable!(),
            }
        }
    });

    thread::sleep(std::time::Duration::from_secs(4));

    Ok(())
}

fn spawn_diagnosis_server() -> Result<Child> {
    todo!("");
}

fn spawn_clients() -> Result<Vec<Child>> {
    todo!("");
}

// fn build_project() -> Result<()> {
//     let mut handle = Command::new("cargo")
//         .arg("build")
//         .arg("--release")
//         .spawn()
//         .context("Failed to spawn 'cargo build --release'")?;
//     let exit_status = handle
//         .wait()
//         .context("Failed to wait 'cargo build --release'")?;
//     if !exit_status.success() {
//         panic!("Build failed, aborting!");
//     } else {
//         Ok(())
//     }
// }
