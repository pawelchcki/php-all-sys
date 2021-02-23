use std::{env, path::PathBuf};

use anyhow::Error;
use cargo_php_sys_build::*;
use clap::Clap;
use rayon::prelude::*;

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
        .or(env::current_dir().ok().map(|p| p.join("src/generated")));
    opts.vendor_path = opts
        .vendor_path
        .or(env::current_dir().ok().map(|p| p.join("vendor")));
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
    [
        (
            output_path.join("php54.rs"),
            vendor_path.join("php-54/include"),
        ),
        (
            output_path.join("php56.rs"),
            vendor_path.join("php-56/include"),
        ),
        (
            output_path.join("php56_zts.rs"),
            vendor_path.join("php-56-zts/include"),
        ),
    ]
    .par_iter()
    .map(|(binding_out, vendor_path)| {
        build_php(
            &binding_out,
            vendor_path,
            PHP5_INCLUDES,
            &wrapper_h,
            None,
        );
    })
    .collect::<()>(); //join

    tempdir.close()?;
    Ok(())
}
