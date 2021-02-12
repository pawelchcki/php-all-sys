use std::{env, fs::File, io::Write, path::PathBuf};

pub static CHECK_C: &[u8] = include_bytes!("check.c");
pub static WRAPPER_H: &[u8] = include_bytes!("wrapper.h");

fn output_file(filename: &str, contents: &[u8]) -> anyhow::Result<PathBuf> {
    let out_path = PathBuf::from(env::var("OUT_DIR")?).join(filename);
    File::create(&out_path)?.write_all(contents)?;
    Ok(out_path)
}

pub fn render_check_c() -> anyhow::Result<PathBuf> {
    output_file("check.c", CHECK_C)
}

pub fn render_wrapper_h() -> anyhow::Result<PathBuf> {
    output_file("wrapper.h", WRAPPER_H)
}
