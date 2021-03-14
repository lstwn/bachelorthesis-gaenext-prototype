mod args;
use anyhow::{Context, Result};
use args::Args;
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::fs;
use std::process::{self, Child, Command};
use std::sync::mpsc;
use std::thread;

fn main() -> Result<()> {
    let args = Args::new();
    println!(
        "INFO: Assuming you have build the project in release mode, \
        run the configurator beforehand and that the generated \
        configurations are present in {:?}",
        args.config_files_path
    );
    println!("INFO: Press CTRL+C to stop diagnosis server and all clients");
    println!(
        "INFO: All logs will appear in this terminal or \
        alternatively per binary in the {:?} folder",
        args.log_files_path
    );
    let mut diagnosis_server_handle = spawn_diagnosis_server(&args)?;
    let client_handles = spawn_clients(&args)?;

    let (tx, rx) = mpsc::channel();
    let mut subscribed_signals = Signals::new(&[SIGINT])?;
    thread::spawn(move || {
        for signal in subscribed_signals.forever() {
            match signal {
                SIGINT => {
                    println!(
                        "Received SIGINT signal ({}), \
                        shutting down clients and diagnosis server..",
                        signal
                    );
                    tx.send(true).unwrap();
                }
                _ => unreachable!(),
            }
        }
    });

    let _ = rx.recv()?;
    diagnosis_server_handle.kill().unwrap();
    for mut client_handle in client_handles {
        client_handle.kill().unwrap();
    }
    Ok(())
}

fn spawn_diagnosis_server(args: &Args) -> Result<Child> {
    let mut config_path = args.config_files_path.clone();
    config_path.push("diagnosisserver");
    config_path.set_extension("yaml");
    let mut log_path = args.log_files_path.clone();
    log_path.push("diagnosisserver");
    log_path.set_extension("log");
    Ok(Command::new("target/release/diagnosisserver")
        .arg(format!("--config={}", config_path.to_str().unwrap()))
        .arg(format!("--log={}", log_path.to_str().unwrap()))
        .arg("-vvvv")
        .spawn()?)
}

fn spawn_clients(args: &Args) -> Result<Vec<Child>> {
    let mut config_path = args.config_files_path.clone();
    config_path.push("clients");
    let mut log_path = args.log_files_path.clone();
    log_path.push("init");
    let mut childs = Vec::new();
    for entry in fs::read_dir(&config_path)? {
        let entry = entry?;
        let config_path = entry.path();
        log_path.set_file_name(config_path.file_stem().unwrap());
        log_path.set_extension("log");
        childs.push(
            Command::new("target/release/client")
                .arg(format!("--config={}", config_path.to_str().unwrap()))
                .arg(format!("--log={}", log_path.to_str().unwrap()))
                .arg("-vvvv")
                .spawn()?,
        );
    }
    Ok(childs)
}
