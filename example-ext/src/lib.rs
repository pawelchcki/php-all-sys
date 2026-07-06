//! A minimal but *real* PHP extension, written once and compiled unchanged
//! against every PHP 8.x ABI exposed by `php-all-sys`.
//!
//! The trick to "supports all 8.x at once" is that a compiled extension can
//! only ever match one PHP ABI (the Zend module API number and build id are
//! checked byte-for-byte at load time). So we don't ship one `.so` that loads
//! everywhere -- we ship one *source file* that the build selects a PHP version
//! for via a Cargo feature, and CI compiles + loads it into every 8.x in turn.
//!
//! Everything version-specific (struct layouts, the `zend_function_entry` size
//! that grew from 32 to 48 bytes in 8.4, constant values, the API number baked
//! into the build id) comes from the per-version module in `php-all-sys`, so
//! the code below never mentions a concrete version.

use core::ffi::{c_char, c_uchar, c_uint, c_ushort, CStr};
use std::ffi::CString;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Select exactly one PHP ABI. The same `php` alias is used everywhere below.
// The `not(...)` guards make the selection exclusive so enabling several
// features (e.g. via `--all-features`) still compiles instead of colliding.
// ---------------------------------------------------------------------------
#[cfg(feature = "php80")]
use php_all_sys::php80 as php;
#[cfg(all(feature = "php81", not(feature = "php80")))]
use php_all_sys::php81 as php;
#[cfg(all(feature = "php82", not(any(feature = "php80", feature = "php81"))))]
use php_all_sys::php82 as php;
#[cfg(all(
    feature = "php83",
    not(any(feature = "php80", feature = "php81", feature = "php82"))
))]
use php_all_sys::php83 as php;
#[cfg(all(
    feature = "php84",
    not(any(
        feature = "php80",
        feature = "php81",
        feature = "php82",
        feature = "php83"
    ))
))]
use php_all_sys::php84 as php;
#[cfg(all(
    feature = "php85",
    not(any(
        feature = "php80",
        feature = "php81",
        feature = "php82",
        feature = "php83",
        feature = "php84"
    ))
))]
use php_all_sys::php85 as php;

#[cfg(not(any(
    feature = "php80",
    feature = "php81",
    feature = "php82",
    feature = "php83",
    feature = "php84",
    feature = "php85"
)))]
compile_error!(
    "select exactly one PHP ABI feature: php80, php81, php82, php83, php84 or php85, \
     e.g. `cargo build --features php82`"
);

// The name PHP shows in `php -m` / `phpinfo()`, the extension version, and the
// userland-visible function names. These are read-only, so plain 'static
// C-string literals are enough.
const EXT_NAME: &CStr = c"php_all_sys_example";
const EXT_VERSION: &CStr = c"0.0.1";
const FN_HELLO: &CStr = c"php_all_sys_hello";
const FN_ZEND_API: &CStr = c"php_all_sys_zend_api";

/// `php_all_sys_hello(): string` -- returns a freshly allocated PHP string
/// naming the PHP version this `.so` was built against.
unsafe extern "C" fn zif_hello(
    _execute_data: *mut php::zend_execute_data,
    return_value: *mut php::zval,
) {
    // zend_strpprintf allocates a new, refcounted (non-interned) zend_string
    // that PHP will free once the returned value is done with.
    let s = php::zend_strpprintf(
        0, // 0 == no length limit
        c"Hello from Rust \xf0\x9f\xa6\x80, built against PHP %s".as_ptr(),
        php::PHP_VERSION.as_ptr() as *const c_char,
    );
    (*return_value).value.str_ = s;
    // ZVAL_STR for a refcounted string: type_info == IS_STRING_EX, expressed
    // from the primitive constants so it stays correct on every version.
    (*return_value).u1.type_info =
        php::IS_STRING | (php::IS_TYPE_REFCOUNTED << php::Z_TYPE_FLAGS_SHIFT);
}

/// `php_all_sys_zend_api(): int` -- returns the Zend module API number the
/// extension was compiled for. Lets CI assert the exact ABI in play.
unsafe extern "C" fn zif_zend_api(
    _execute_data: *mut php::zend_execute_data,
    return_value: *mut php::zval,
) {
    // RETURN_LONG: set the value union then tag the zval as a plain long.
    (*return_value).value.lval = php::ZEND_MODULE_API_NO as php::zend_long;
    (*return_value).u1.type_info = php::IS_LONG;
}

/// Rendered inside `phpinfo()` / `php --ri php_all_sys_example`.
unsafe extern "C" fn minfo(_module: *mut php::zend_module_entry) {
    php::php_info_print_table_start();
    php::php_info_print_table_row(2, c"php-all-sys example".as_ptr(), c"enabled".as_ptr());
    php::php_info_print_table_row(
        2,
        c"Built against PHP".as_ptr(),
        php::PHP_VERSION.as_ptr() as *const c_char,
    );
    php::php_info_print_table_end();
}

/// Build a `zend_function_entry`, zero-initialising it first so version-only
/// fields (8.4+ added `frameless_function_infos` and `doc_comment`) stay NULL.
unsafe fn make_fe(
    name: &'static CStr,
    handler: php::zif_handler,
    arg_info: *const php::zend_internal_arg_info,
    num_args: u32,
) -> php::zend_function_entry {
    let mut e: php::zend_function_entry = core::mem::zeroed();
    e.fname = name.as_ptr();
    e.handler = handler;
    e.arg_info = arg_info;
    e.num_args = num_args;
    e
}

/// Construct the module description PHP asks for via `get_module()`.
///
/// PHP mutates the module entry after registration (module_number, handle,
/// ...), so it must outlive the call: every piece is leaked to `'static`.
unsafe fn build_module() -> *mut php::zend_module_entry {
    // A single zeroed arg-info entry is `ZEND_BEGIN_ARG_INFO_EX(0, ...)`:
    // zero required args, no declared return type. Both zero-arg functions can
    // share it. (The all-zero terminator below is `ZEND_FE_END`.)
    let arg_info: &'static [php::zend_internal_arg_info; 1] =
        Box::leak(Box::new([core::mem::zeroed()]));
    let arg_info_ptr = arg_info.as_ptr();

    let functions: &'static [php::zend_function_entry; 3] = Box::leak(Box::new([
        make_fe(FN_HELLO, Some(zif_hello), arg_info_ptr, 0),
        make_fe(FN_ZEND_API, Some(zif_zend_api), arg_info_ptr, 0),
        core::mem::zeroed(), // ZEND_FE_END
    ]));

    // The build id is compared with `strcmp` against the host PHP's
    // ZEND_MODULE_BUILD_ID; a mismatch makes dl() refuse the module. On Linux
    // (NTS, non-debug) that string is exactly "API<ZEND_MODULE_API_NO>,NTS".
    // ZEND_BUILD_TS is the byte string b",NTS\0", so we drop its trailing NUL.
    let ts = core::str::from_utf8(&php::ZEND_BUILD_TS[..php::ZEND_BUILD_TS.len() - 1]).unwrap();
    let build_id: &'static CString = Box::leak(Box::new(
        CString::new(format!("API{}{}", php::ZEND_MODULE_API_NO, ts)).unwrap(),
    ));

    // Zero everything (== STANDARD_MODULE_PROPERTIES) then fill the header.
    let mut m: php::zend_module_entry = core::mem::zeroed();
    m.size = core::mem::size_of::<php::zend_module_entry>() as c_ushort;
    m.zend_api = php::ZEND_MODULE_API_NO as c_uint;
    m.zend_debug = php::ZEND_DEBUG as c_uchar;
    m.zts = php::USING_ZTS as c_uchar;
    m.name = EXT_NAME.as_ptr();
    m.functions = functions.as_ptr();
    m.info_func = Some(minfo);
    m.version = EXT_VERSION.as_ptr();
    m.build_id = build_id.as_ptr();

    Box::into_raw(Box::new(m))
}

/// The one symbol PHP looks up (`dlsym(handle, "get_module")`) when loading a
/// shared extension. Called once; we memoise the leaked pointer.
///
/// # Safety
/// Called by the PHP/Zend engine during module load with no other threads
/// touching the module registry.
#[no_mangle]
pub unsafe extern "C" fn get_module() -> *mut php::zend_module_entry {
    static MODULE: OnceLock<usize> = OnceLock::new();
    *MODULE.get_or_init(|| unsafe { build_module() } as usize) as *mut php::zend_module_entry
}
