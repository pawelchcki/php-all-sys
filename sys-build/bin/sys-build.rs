use std::{env, path::PathBuf};

use anyhow::Error;
use cargo_php_sys_build::*;
use clap::Clap;
use rayon::prelude::*;

const VERSIONS: &[(&str, &str)] = &[
    ("php54.rs", "php-54/include/php5"),
    ("php56.rs", "php-56/include/php5"),
    ("php56_zts.rs", "php-56-zts/include/php5"),
    ("php70.rs", "php-70/include/php/20151012"),
    ("php71.rs", "php-71/include/php/20160303"),
    ("php72.rs", "php-72/include/php/20170718"),
    ("php73.rs", "php-73/include/php/20180731"),
    ("php74.rs", "php-74/include/php/20190902"),
    ("php80.rs", "php-80/include/php/20200930"),
];

#[derive(Clap, Debug)]
#[clap(version=env!("CARGO_PKG_VERSION"))]
struct Opts {
    // Cargo command name when run via Cargo
    cargo_command_name: Option<String>,

    /// Directory to output generated files. Defaults to current working directory + src/generated
    #[clap(short, long)]
    output_path: Option<PathBuf>,

    /// Directory with vendored PHP include files. Defaults to current working directory + vendor
    #[clap(short, long)]
    vendor_path: Option<PathBuf>,
}

fn parse() -> Opts {
    let mut opts: Opts = Opts::parse();
    opts.output_path = opts
        .output_path
        .or_else(|| env::current_dir().ok().map(|p| p.join("src/generated")));
    opts.vendor_path = opts
        .vendor_path
        .or_else(|| env::current_dir().ok().map(|p| p.join("vendor")));
    opts
}


fn main() -> anyhow::Result<()> {
    let opts = parse();
    let output_path = opts.output_path.unwrap();
    if !output_path.exists() {
        return Err(Error::msg(format!(
            "Output path: {:?} not found",
            output_path
        )));
    }

    let vendor_path = opts.vendor_path.unwrap();

    if !vendor_path.exists() {
        return Err(Error::msg(format!(
            "Vendor path: {:?} not found",
            vendor_path
        )));
    }

    let tempdir = tempdir::TempDir::new("cargo-php-sys-build")?;
    env::set_var("OUT_DIR", tempdir.path().to_str().unwrap());

    let wrapper_h = render_wrapper_h()?;
    VERSIONS
        .iter()
        .map(|(out_file, include_root)| {
            (output_path.join(out_file), vendor_path.join(include_root))
        })
        .par_bridge()
        .map(|(binding_out, root_include_path)| {
            build_php(
                &binding_out,
                &root_include_path,
                PHP_INCLUDES,
                &wrapper_h,
                None,
            );
        })
        .collect::<()>(); //join

    tempdir.close()?;
    Ok(())
}
