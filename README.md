# Prototype for Thesis "Secure and Private Multi-Party Event Detection" @ TUM

Author: Leo Stewen
Advisor: Mark Gall
Supervisor: Prof. Claudia Eckert

## Requirements

Make sure to have `rust` and `cargo` installed.
Tested only on Linux and with `rust >= 1.50.0`.
Comes with no guarantees for other platforms or older rust versions.

## Onboarding

```
cargo build --release
cargo run --release --bin configurator -- emit-default-config
cargo run --release --bin configurator -- generate-configs
cargo run --release --bin runner
```

## Details

All binaries come with a cli interface.
Check `cargo run --release --bin <binary> -- --help` for cli argument help.

For playing around with different setups, i.e. graphs, we recommend to
run `cargo run --release --bin configurator -- emit-default-config`
and then modify the config in `configs/configurator.yaml`.
Afterwards, run `cargo run --release --bin configurator -- generate-configs`
to generate the client configs and diagnosis server config.
Finally, use `cargo run --release --bin runner` to start the diagnosis server
and all clients locally on your machine.
Logs will be written into the `logs` folder.
