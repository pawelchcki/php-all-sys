mod build_files;
mod phpgen;

pub use build_files::*;
pub use phpgen::*;

pub static PHP5_INCLUDES: &[&str] = &["php5/Zend", "php5", "php5/TSRM", "php5/main"];
