mod args;
use anyhow::{Context, Result};
use args::Args;
use crossbeam::channel::unbounded;
use crossbeam::channel::{Receiver, Sender};
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

const DIVIDER: &'static str = "-------------------------------------------";

fn main() -> Result<()> {
    let args = Args::new();
    println!("Building the project in release mode\n{}", DIVIDER);
    let exit_status = build_project().context("Error spawning build process")?;
    if !exit_status.success() {
        println!("{}\nError building the project in release mode, aborting", DIVIDER);
        std::process::exit(exit_status.code().unwrap_or(1));
    }
    println!(
        "{}\nAssuming you have run the configurator beforehand and that the generated \
        (and up-to-date) configurations are present in {:?}",
        DIVIDER,
        args.config_files_path
    );
    println!("Press CTRL+C to stop diagnosis server and all clients");
    println!(
        "All logs will appear in this terminal (not necessarily chronologically ordered!) \
        or alternatively per binary in the {:?} folder\n{}",
        args.log_files_path,
        DIVIDER
    );
    fs::create_dir_all(&args.log_files_path)?;

    let (output_tx, output_rx) = unbounded::<String>();
    let (termination_request_tx, termination_request_rx) = unbounded::<()>();
    let (termination_done_tx, termination_done_rx) = unbounded::<()>();
    let subprocess_channels = SubprocessChannels {
        output_tx,
        termination_request_rx: termination_request_rx.clone(),
        termination_done_tx,
    };

    let mut subprocesses = spawn_diagnosis_server(&args, subprocess_channels.clone())
        .context("Error launching diagnosis server")?;
    // let's wait a bit until the diagnosis server is up and running
    thread::sleep(Duration::from_secs(2));
    subprocesses +=
        spawn_clients(&args, subprocess_channels.clone()).context("Error launching clients")?;

    let mut subscribed_signals = Signals::new(&[SIGINT])?;
    thread::spawn(move || {
        for signal in subscribed_signals.forever() {
            match signal {
                SIGINT => {
                    println!(
                        "{}\nReceived SIGINT signal ({}), \
                        shutting down clients and diagnosis server..",
                        DIVIDER,
                        signal
                    );
                    termination_request_tx.send(()).unwrap();
                }
                _ => unreachable!(),
            }
        }
    });

    loop {
        match output_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(output_line) => print!("{}", output_line),
            Err(_) => {
                if !termination_request_rx.is_empty() {
                    for _ in 0..subprocesses {
                        let _ = termination_done_rx.recv();
                    }
                    return Ok(());
                }
            }
        };
    }
}

#[derive(Clone)]
struct SubprocessChannels {
    output_tx: Sender<String>,
    termination_request_rx: Receiver<()>,
    termination_done_tx: Sender<()>,
}

fn spawn_diagnosis_server(args: &Args, channels: SubprocessChannels) -> Result<usize> {
    let mut config_path = args.config_files_path.clone();
    config_path.push("diagnosisserver");
    config_path.set_extension("yaml");
    let mut log_path = args.log_files_path.clone();
    log_path.push("diagnosisserver");
    log_path.set_extension("log");
    monitor_subprocess(
        args,
        String::from("diagnosis server"),
        Command::new("target/release/diagnosisserver")
            .arg(format!("--config={}", config_path.to_str().unwrap()))
            .arg(format!("--log={}", log_path.to_str().unwrap()))
            .arg(format!("-{}", args.log_level))
            .stderr(Stdio::piped())
            .spawn()?,
        channels.clone(),
    );
    Ok(1)
}

fn spawn_clients(args: &Args, channels: SubprocessChannels) -> Result<usize> {
    let mut config_path = args.config_files_path.clone();
    config_path.push("clients");
    let mut log_path = args.log_files_path.clone();
    log_path.push("init");
    let mut count = 0;
    for entry in fs::read_dir(&config_path)? {
        let entry = entry?;
        let config_path = entry.path();
        log_path.set_file_name(config_path.file_stem().unwrap());
        log_path.set_extension("log");
        monitor_subprocess(
            args,
            String::from(format!(
                "client {}",
                config_path.file_stem().unwrap().to_str().unwrap()
            )),
            Command::new("target/release/client")
                .arg(format!("--config={}", config_path.to_str().unwrap()))
                .arg(format!("--log={}", log_path.to_str().unwrap()))
                .arg(format!("-{}", args.log_level))
                .stderr(Stdio::piped())
                .spawn()?,
            channels.clone(),
        );
        count += 1;
    }
    Ok(count)
}

fn monitor_subprocess(
    args: &Args,
    name: String,
    mut child: Child,
    channels: SubprocessChannels,
) -> () {
    let log_refresh_rate = args.log_refresh_rate;
    thread::spawn(move || {
        let stderr = child.stderr.take().unwrap();
        let mut stderr = BufReader::new(stderr);
        loop {
            // TODO: batched read instead of line by line read
            if !channels.termination_request_rx.is_empty() {
                break;
            }
            let mut output_line = String::new();
            let bytes = stderr.read_line(&mut output_line).unwrap();
            if bytes == 0 {
                // update rate for new logs after an EOF
                // (i.e. a logging break)
                thread::sleep(Duration::from_millis(log_refresh_rate));
                continue;
            }
            channels.output_tx.send(output_line).unwrap();
        }
        let _ = child.kill();
        println!("Killed subprocess '{}' with ID '{}'", name, child.id());
        channels.termination_done_tx.send(()).unwrap();
    });
}

fn build_project() -> Result<std::process::ExitStatus> {
    Ok(Command::new("cargo")
        .arg("build")
        .arg("--release")
        .status()?)
}
