use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, SubCommand};
use std::path::PathBuf;

#[derive(Debug)]
pub enum Args {
    GenerateConfigs(GenerateConfigsArgs),
    EmitDefaultConfig(EmitDefaultConfigArgs),
}

#[derive(Debug)]
pub struct GenerateConfigsArgs {
    pub config_file_path: PathBuf,
    pub config_output_path: PathBuf,
}

#[derive(Debug)]
pub struct EmitDefaultConfigArgs {
    pub config_file_path: PathBuf,
}

impl Args {
    const GENERATE_CONFIGS: &'static str = "generate-configs";
    const EMIT_DEFAULT_CONFIG: &'static str = "emit-default-config";
    const CONFIG_FILE_PATH: &'static str = "CONFIG_FILE_PATH";
    const CONFIG_OUTPUT_PATH: &'static str = "CONFIG_OUTPUT_PATH";
    const CONFIG_FILE_PATH_DEFAULT: &'static str = "configs/configurator.yaml";
    const CONFIG_OUTPUT_PATH_DEFAULT: &'static str = "configs";

    pub fn new() -> Self {
        let matches = App::new(crate_name!())
            .version(crate_version!())
            .author(crate_authors!())
            .about(crate_description!())
            .subcommand(
                SubCommand::with_name(Self::GENERATE_CONFIGS)
                    .about("Generates the client configs according to the yaml config file")
                    .arg(
                        Arg::with_name(Self::CONFIG_FILE_PATH)
                            .short("c")
                            .long("config")
                            .value_name("FILE")
                            .default_value(Self::CONFIG_FILE_PATH_DEFAULT)
                            .help("Sets the input yaml config file"),
                    )
                    .arg(
                        Arg::with_name(Self::CONFIG_OUTPUT_PATH)
                        .short("o")
                        .long("output")
                        .value_name("DIRECTORY")
                        .default_value(Self::CONFIG_OUTPUT_PATH_DEFAULT)
                        .help("Sets the output directory to put all generated configs into"),
                    ),
            )
            .subcommand(
                SubCommand::with_name(Self::EMIT_DEFAULT_CONFIG)
                    .about("Emits a possible default configuration for the configurator")
                    .arg(
                        Arg::with_name(Self::CONFIG_FILE_PATH)
                            .short("o")
                            .long("output")
                            .value_name("FILE")
                            .default_value(Self::CONFIG_FILE_PATH_DEFAULT)
                            .help("Sets the output yaml config file"),
                        ),
            )
            .get_matches();

        match matches.subcommand_name() {
            Some(Self::GENERATE_CONFIGS) => {
                let matches = matches.subcommand_matches(Self::GENERATE_CONFIGS).unwrap();
                Args::GenerateConfigs(GenerateConfigsArgs {
                    config_file_path: matches.value_of(Self::CONFIG_FILE_PATH).unwrap().into(),
                    config_output_path: matches.value_of(Self::CONFIG_OUTPUT_PATH).unwrap().into(),
                })
            }
            Some(Self::EMIT_DEFAULT_CONFIG) => {
                let matches = matches.subcommand_matches(Self::EMIT_DEFAULT_CONFIG).unwrap();
                Args::EmitDefaultConfig(EmitDefaultConfigArgs {
                    config_file_path: matches.value_of(Self::CONFIG_FILE_PATH).unwrap().into(),
                })
            }
            None => panic!("Please specify which subcommand to use. See --help for usage."),
            _ => panic!("Invalid subcommand."),
        }
    }
}
