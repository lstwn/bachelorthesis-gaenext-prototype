use exposurelib::args::{Args, crate_name, crate_version, crate_authors, crate_description};
use exposurelib::logger;

fn main() {
    let args = Args::new(crate_name!(), crate_version!(), crate_authors!(), crate_description!());
    logger::setup_logger(&args.config_file_path, args.log_level);
    println!("Hello, world!");
}
