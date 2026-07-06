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
        // Emit struct-layout checks. Modern bindgen emits these as compile-time
        // `const _` assertions (size_of/align_of/offset_of!), so layout is
        // verified on every `cargo check`/`build` - unlike the 0.57 output, whose
        // runtime layout tests dereferenced a null pointer and abort on current Rust.
        .layout_tests(true)
        // NOTE: bindgen >=0.60 anchors allowlist patterns and rejects a leading
        // `(?i)` inline flag, so the original case-insensitive patterns matched
        // nothing. PHP/Zend C symbols are only ever lower- or UPPER-snake-case
        // (lowercase functions/types, UPPERCASE macro constants), so each rule is
        // listed in both cases instead of relying on `(?i)`.
        .allowlist_function(r"php_.*")
        .allowlist_function(r"[_]?zend.*")
        .allowlist_function(r".*zend")
        .allowlist_type(r"zend.*")
        .allowlist_type(r"ZEND.*")
        .allowlist_var(r"[_]?zend.*")
        .allowlist_var(r"[_]?ZEND.*")
        .allowlist_var(r"php.*")
        .allowlist_var(r"PHP.*")
        .allowlist_var(r"[_]?is_.*")
        .allowlist_var(r"[_]?IS_.*")
        .allowlist_var(r"configure_.*")
        .allowlist_var(r"CONFIGURE_.*")
        .allowlist_var(r"gc_.*")
        .allowlist_var(r"GC_.*")
        .allowlist_var(r"using.*")
        .allowlist_var(r"USING.*")
        .allowlist_var(r"z_.*")
        .allowlist_var(r"Z_.*")
        .allowlist_var(r"zmsg")
        .allowlist_var(r"ZMSG")
        .allowlist_var(r"module.*")
        .allowlist_var(r"MODULE.*")
        .allowlist_var(r"hash_.*")
        .allowlist_var(r"HASH_.*")
        .allowlist_var(r"debug.*")
        .allowlist_var(r"DEBUG.*")
        .allowlist_var(r".*_globals")
        .allowlist_var(r".*_GLOBALS")
        .allowlist_var(r"module_registry")
        .allowlist_var(r"MODULE_REGISTRY")
        .allowlist_var(r"empty_fcall.*")
        .allowlist_var(r"EMPTY_FCALL.*")
        .allowlist_var(r"std_object_handlers")
        .allowlist_var(r"STD_OBJECT_HANDLERS")
        .allowlist_var(r".*zval.*")
        .allowlist_var(r".*ZVAL.*")
        .raw_line("#![allow(clippy::all)]") // trusting bindgen - lets disable clippy reported things, as we can't do anything
        .raw_line("#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case, clashing_extern_declarations)]\n")
        .clang_args(clang_args)
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(bindings_rs)
        .expect("Couldn't write bindings!");
}
