use std::{env, path::PathBuf};

use php_build::*;
use rayon::prelude::*;

fn main() {
    let out_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src/generated");
    // out_path.join("bindings.rs");
    let wrapper_h = render_wrapper_h().unwrap();
    let check_c = render_check_c().unwrap();

    [
        (out_path.join("php54.rs"), "php-54/include"),
        (out_path.join("php56.rs"), "php-56/include"),
        (out_path.join("php56_zts.rs"), "php-56-zts/include"),
    ]
    .par_iter()
    .map(|(binding_out, root_path)| {
        build_php(
            &binding_out,
            root_path,
            PHP5_INCLUDES,
            &wrapper_h,
            Some(&check_c),
        );
    })
    .collect::<()>(); //join
}
