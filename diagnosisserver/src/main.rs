use exposurelib::args::{Args, crate_name, crate_version, crate_authors, crate_description};
use exposurelib::logger;

fn main() {
    let args = Args::new(crate_name!(), crate_version!(), crate_authors!(), crate_description!());
    logger::setup_logger(&args.log_file_path, args.log_level, String::from("ds"));
    logger::info!("Hello from Diagnosis Server");

    std::thread::sleep(std::time::Duration::from_secs(10));
    logger::info!("Hello from Diagnosis Server again");
}
