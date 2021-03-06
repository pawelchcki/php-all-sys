use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct IgnoreMacros(HashSet<String>);

impl bindgen::callbacks::ParseCallbacks for IgnoreMacros {
    fn will_parse_macro(&self, name: &str) -> bindgen::callbacks::MacroParsingBehavior {
        if self.0.contains(name) {
            bindgen::callbacks::MacroParsingBehavior::Ignore
        } else {
            bindgen::callbacks::MacroParsingBehavior::Default
        }
    }
}

fn build_ignored_macros() -> IgnoreMacros {
    IgnoreMacros(
        vec![
            "FP_INFINITE".into(),
            "FP_NAN".into(),
            "FP_NORMAL".into(),
            "FP_SUBNORMAL".into(),
            "FP_ZERO".into(),
            "FP_INT_UPWARD".into(),
            "FP_INT_DOWNWARD".into(),
            "FP_INT_TOWARDZERO".into(),
            "FP_INT_TONEARESTFROMZERO".into(),
            "FP_INT_TONEAREST".into(),
            "IPPORT_RESERVED".into(),
        ]
        .into_iter()
        .collect(),
    )
}

fn run_checks<T>(includes: T, check_path: &str)
where
    T: IntoIterator,
    T::Item: AsRef<Path>,
{
    let check = cc::Build::new()
        .file(check_path)
        .includes(includes)
        .expand();
    let check = String::from_utf8(check).unwrap();

    if check.contains("CHECK_DEBUG_ZEND_TRUE") {
        println!("rustc-cfg=debug_zend");
    }

    if check.contains("CHECK_USING_ZTS_TRUE") {
        println!("rustc-cfg=using_zts");
    }
}

pub fn build_php(
    bindings_rs: &PathBuf,
    root_path: &PathBuf,
    include_paths: &[&str],
    wrapper_h: &PathBuf,
    check_c_path: Option<&PathBuf>,
) {
    println!("cargo:rerun-if-changed={}", wrapper_h.to_str().unwrap());
    let include_paths = include_paths
        .iter()
        .map(|v| {
            root_path
                .join(v)
                .canonicalize()
                .unwrap_or_else(|_| panic!("Include path not found : {:?}", root_path.join(v)))
        })
        .map(|v| String::from(v.to_str().unwrap()));

    if let Some(p) = check_c_path {
        println!("cargo:rerun-if-changed={}", p.to_str().unwrap());
        run_checks(include_paths.clone(), p.to_str().unwrap());
    }

    let clang_args = include_paths.map(|v| format!("-I{}", v));

    let bindings = bindgen::Builder::default()
        .header(wrapper_h.to_str().unwrap())
        .parse_callbacks(Box::new(build_ignored_macros()))
        .whitelist_function(r"(?i)php_.*")
        .whitelist_function(r"(?i)[_]?zend.*")
        .whitelist_function(r"(?i).*zend")
        .whitelist_type(r"(?i)zend.*")
        .whitelist_var(r"(?i)[_]?zend.*")
        .whitelist_var(r"(?i)php.*")
        .whitelist_var(r"(?i)[_]?is_.*")
        .whitelist_var(r"(?i)configure_.*")
        .whitelist_var(r"(?i)gc_.*")
        .whitelist_var(r"(?i)using.*")
        .whitelist_var(r"(?i)z_.*")
        .whitelist_var(r"(?i)zmsg")
        .whitelist_var(r"(?i)module.*")
        .whitelist_var(r"(?i)hash_.*")
        .whitelist_var(r"(?i)debug.*")
        .whitelist_var(r"(?i).*_globals")
        .whitelist_var(r"(?i)module_registry")
        .whitelist_var(r"(?i)empty_fcall.*")
        .whitelist_var(r"(?i)std_object_handlers")
        .whitelist_var(r"(?i).*zval.*")
        .whitelist_var(r"(?i)")
        .raw_line("#![allow(clippy::all)]") // trusting bindgen - lets disable clippy reported things, as we can't do anything
        .raw_line("#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case, clashing_extern_declarations)]\n")
        .clang_args(clang_args)
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(bindings_rs)
        .expect("Couldn't write bindings!");
}
