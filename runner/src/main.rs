mod args;
use anyhow::{Context, Result};
use args::Args;
use crossbeam::channel::unbounded;
use crossbeam::channel::{Receiver, Sender};
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Stdio, Child, Command};
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
    fs::create_dir_all(&args.log_files_path)?;

    let (output_tx, output_rx) = unbounded::<String>();
    let (term_tx, term_rx) = unbounded::<()>();

    let _ = spawn_diagnosis_server(&args, output_tx.clone(), term_rx.clone())
        .context("Error launching diagnosis server")?;
    let _ = spawn_clients(&args, output_tx.clone(), term_rx.clone())
        .context("Error launching clients")?;

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
                    term_tx.send(()).unwrap();
                }
                _ => unreachable!(),
            }
        }
    });

    loop {
        match output_rx.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(output_line) => print!("{}", output_line),
            Err(_) => {
                // Optional: use another channel to await kill of other
                // subprocesses
                if !term_rx.is_empty() {
                    return Ok(());
                }
            }
        };
    }
}

fn spawn_diagnosis_server(
    args: &Args,
    output_tx: Sender<String>,
    term_rx: Receiver<()>,
) -> Result<()> {
    let mut config_path = args.config_files_path.clone();
    config_path.push("diagnosisserver");
    config_path.set_extension("yaml");
    let mut log_path = args.log_files_path.clone();
    log_path.push("diagnosisserver");
    log_path.set_extension("log");
    monitor_subprocess(
        Command::new("target/release/diagnosisserver")
            .arg(format!("--config={}", config_path.to_str().unwrap()))
            .arg(format!("--log={}", log_path.to_str().unwrap()))
            .arg(format!("-{}", args.log_level))
            .stderr(Stdio::piped())
            .spawn()?,
        output_tx,
        term_rx,
    );
    Ok(())
}

fn spawn_clients(args: &Args, output_tx: Sender<String>, term_rx: Receiver<()>) -> Result<()> {
    let mut config_path = args.config_files_path.clone();
    config_path.push("clients");
    let mut log_path = args.log_files_path.clone();
    log_path.push("init");
    for entry in fs::read_dir(&config_path)? {
        let entry = entry?;
        let config_path = entry.path();
        log_path.set_file_name(config_path.file_stem().unwrap());
        log_path.set_extension("log");
        monitor_subprocess(
            Command::new("target/release/client")
                .arg(format!("--config={}", config_path.to_str().unwrap()))
                .arg(format!("--log={}", log_path.to_str().unwrap()))
                .arg(format!("-{}", args.log_level))
                .stderr(Stdio::piped())
                .spawn()?,
            output_tx.clone(),
            term_rx.clone(),
        );
    }
    Ok(())
}

fn monitor_subprocess(mut child: Child, output_tx: Sender<String>, term_rx: Receiver<()>) -> () {
    thread::spawn(move || {
        let stderr = child.stderr.take().unwrap();
        let mut stderr = BufReader::new(stderr);
        loop {
            if !term_rx.is_empty() {
                println!("Shutting down subprocess with ID {}", child.id());
                break;
            }
            let mut output_line = String::new();
            let bytes = stderr.read_line(&mut output_line).unwrap();
            if bytes == 0 {
                // update rate of one second for new logs after an EOF
                thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
            output_tx.send(output_line).unwrap();
        }
        let _ = child.kill();
    });
}
