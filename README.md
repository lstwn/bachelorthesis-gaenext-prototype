# Prototype for Thesis "Secure and Private Multi-Party Event Detection" @ TUM

|------------|--------------------------|
| Author     | Leo Stewen               |
| Advisor    | Mark Gall                |
| Supervisor | Prof. Dr. Claudia Eckert |

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

## Folder Conventions

Although the paths `configs` and `logs` are configurable,
they default to these two in all command line interfaces and it
is recommended to keep them like this.

However, the structure within the `configs` folder looks like this
by convention:
```
configs
├── clients
│  ├── p0.yaml
│  ├── p1.yaml
│  ├── p2.yaml
│  ├── p3.yaml
│  └── p4.yaml
├── configurator.dot
├── configurator.yaml
└── diagnosisserver.yaml
```

The `logs` folder is flat by convention.
```
logs
├── diagnosisserver.log
├── p0.log
├── p1.log
├── p2.log
├── p3.log
└── p4.log
```
