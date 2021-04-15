# Prototype for Thesis "Secure and Private Multi-Party Event Detection" @ TUM

| Role       | Name                     |
|------------|--------------------------|
| Author     | Leo Stewen               |
| Advisor    | Mark Gall                |
| Supervisor | Prof. Dr. Claudia Eckert |

## Workspace Overview

```
├── configurator     // crate 1: generates configs
├── runner           // crate 2: spawns clients and diagnosisserver
├── client           // crate 3: GAENEXT client
├── diagnosisserver  // crate 4: GAENEXT diagnosis server
├── exposurelib      // crate 5: shared library with primitives etc.
├── logs             // generated folder for logs
├── configs          // generated folder for configs
```

## Requirements

Make sure to have `rust` and `cargo` installed.
Tested only on Linux and with `rust >= 1.50.0`.
Comes with no guarantees for other platforms or older rust versions.

## Quick Onboarding

```
cargo build --release
cargo run --release --bin configurator -- emit-default-config
cargo run --release --bin configurator -- generate-configs
cargo run --release --bin runner
```

## Details

All binaries come with a cli interface.
Check `cargo run --release --bin <binary> -- --help` for cli argument help.

For playing around with different setups, i.e. graphs, I recommend to
run `cargo run --release --bin configurator -- emit-default-config`
and then modify the config in `configs/configurator.yaml`.
Alternatively, open `configurator/src/config.rs` and modify the default
config with the given helper functions directly in Rust and then use
`cargo run --release --bin configurator -- emit-default-config` to spit out
the new "default" config.

Afterward, run `cargo run --release --bin configurator -- generate-configs`
to generate the client configs and diagnosis server config from the
`configs/configurator.yaml` file.
Finally, use `cargo run --release --bin runner` to start the diagnosis server
and all clients locally on your machine.
Logs will be written into the `logs` folder and appear on your terminal.

## Folder Conventions

Although the paths `configs` and `logs` are configurable,
they default to these two in all cli interfaces and it
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

