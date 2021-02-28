use exposurelib::args::{Args, crate_name, crate_version, crate_authors, crate_description};

fn main() {
    let args = Args::new(crate_name!(), crate_version!(), crate_authors!(), crate_description!());
    println!("Hello, world!");
}
