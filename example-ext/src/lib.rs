//! A minimal but *real* PHP extension: **one** `.so` that loads unchanged into
//! every PHP 8.x (8.0–8.5).
//!
//! When PHP `dlopen`s an extension it validates two fields of the module entry
//! byte-for-byte -- `zend_api` (`==`) and `build_id` (`strcmp`) -- so a module
//! with those values baked in at compile time loads on exactly one PHP minor.
//! Instead of baking them in, we read the *host* interpreter's own values at
//! load time: PHP registers its built-in modules (ext/standard, Core, ...) into
//! the exported `module_registry` HashTable before it loads us, and each carries
//! the running engine's exact `zend_api`/`build_id`/`zts`/`zend_debug`/`size`.
//! We read one and echo those fields, so the load-gate matches on every version.
//!
//! The only ABI shape that actually changed across 8.x is `zend_function_entry`
//! (32 bytes on 8.0-8.3, 48 on 8.4+, which appended two fields); we pick its
//! stride at runtime from the detected API number. Every other type we touch
//! (`zend_module_entry`, `zval`, `zend_internal_arg_info`, ...) is byte-identical
//! across 8.x, so a single binding is ABI-correct for all of them -- we use the
//! newest, `php_all_sys::php85`, and never name a concrete version below.

use core::ffi::{c_char, c_uint, CStr};
use core::ptr;
use std::alloc::{alloc_zeroed, handle_alloc_error, Layout};
use std::ffi::CString;
use std::sync::OnceLock;

use php_all_sys::php85 as php;

// The name PHP shows in `php -m` / `phpinfo()`, the extension version, and the
// userland-visible function names. These are read-only, so plain 'static
// C-string literals are enough.
const EXT_NAME: &CStr = c"php_all_sys_example";
const EXT_VERSION: &CStr = c"0.0.1";
const FN_HELLO: &CStr = c"php_all_sys_hello";
const FN_ZEND_API: &CStr = c"php_all_sys_zend_api";

// PHP 8.4 (API 20240924) grew `zend_function_entry` from 32 to 48 bytes. Any
// host at or above this API number uses the wider stride; older ones the narrow.
const API_ZEND_FUNCTION_ENTRY_48: c_uint = 20240924;
const FE_SIZE_NARROW: usize = 32; // PHP 8.0-8.3
const FE_SIZE_WIDE: usize = 48; // PHP 8.4+

// By the time PHP calls our `get_module()` it has already registered its
// built-in modules into the exported `module_registry` HashTable. Every module
// there is stamped (via STANDARD_MODULE_HEADER) with the running engine's exact
// `zend_api`/`build_id`/`zts`/`zend_debug`/`size`, so *any* of them reveals the
// host ABI. We look these up by their (lower-cased) registry keys and take the
// first that resolves. `standard` and `core` cannot be compiled out, so at
// least one always hits on a real PHP.
//
// (The internal `basic_functions_module` symbol itself is not usable: official
// PHP builds are stripped PIEs that don't export it in `.dynsym`. `module_registry`
// *is* exported -- it is `ZEND_API`, like every Zend function we already call.)
const HOST_MODULE_NAMES: [&CStr; 3] = [
    c"standard", // ext/standard -- cannot be disabled
    c"core",     // the Zend engine's own "Core" module
    c"date",     // ext/date
];

/// The running interpreter's own `zend_module_entry`, resolved once from
/// `module_registry` and memoised. `get_module()` is the first thing PHP calls
/// after `dlopen`, by which point the built-in modules are long since
/// registered, so the lookup always succeeds on a real PHP. Returns `None` only
/// in the impossible case that none of [`HOST_MODULE_NAMES`] is registered.
fn host_module() -> Option<&'static php::zend_module_entry> {
    // Store the raw pointer as `usize` so the `OnceLock` is trivially `Sync`.
    static HOST: OnceLock<usize> = OnceLock::new();
    let addr = *HOST.get_or_init(|| unsafe { resolve_host_module() });
    // SAFETY: a non-null result points at a `zend_module_entry` owned by the
    // host module registry, which lives for the whole process, hence 'static.
    (addr != 0).then(|| unsafe { &*(addr as *const php::zend_module_entry) })
}

/// Look up each of [`HOST_MODULE_NAMES`] in `module_registry`; return the
/// address of the first module found, or 0 if none are.
unsafe fn resolve_host_module() -> usize {
    for name in HOST_MODULE_NAMES {
        // Registry keys are the module names without their trailing NUL.
        let zv = php::zend_hash_str_find(
            ptr::addr_of!(php::module_registry),
            name.as_ptr(),
            name.to_bytes().len(),
        );
        if !zv.is_null() {
            // The stored zval is an IS_PTR to the module's zend_module_entry.
            let entry = (*zv).value.ptr as usize;
            if entry != 0 {
                return entry;
            }
        }
    }
    0
}

/// The Zend module API number of the PHP actually running us, detected from the
/// host module entry. 0 if detection failed (the module then won't have loaded).
fn host_zend_api() -> c_uint {
    host_module().map_or(0, |m| m.zend_api)
}

/// `php_all_sys_hello(): string` -- returns a freshly allocated PHP string
/// naming the Zend API of the PHP that loaded this single, universal `.so`.
unsafe extern "C" fn zif_hello(
    _execute_data: *mut php::zend_execute_data,
    return_value: *mut php::zval,
) {
    // zend_strpprintf allocates a new, refcounted (non-interned) zend_string
    // that PHP will free once the returned value is done with.
    let s = php::zend_strpprintf(
        0, // 0 == no length limit
        c"Hello from Rust \xf0\x9f\xa6\x80 \xe2\x80\x94 single .so, loaded into PHP with Zend API %u"
            .as_ptr(),
        host_zend_api(),
    );
    (*return_value).value.str_ = s;
    // ZVAL_STR for a refcounted string: type_info == IS_STRING_EX, expressed
    // from the primitive constants so it stays correct on every version.
    (*return_value).u1.type_info =
        php::IS_STRING | (php::IS_TYPE_REFCOUNTED << php::Z_TYPE_FLAGS_SHIFT);
}

/// `php_all_sys_zend_api(): int` -- returns the Zend module API number detected
/// from the host at load time. Lets CI assert the exact ABI in play.
unsafe extern "C" fn zif_zend_api(
    _execute_data: *mut php::zend_execute_data,
    return_value: *mut php::zval,
) {
    // RETURN_LONG: set the value union then tag the zval as a plain long.
    (*return_value).value.lval = host_zend_api() as php::zend_long;
    (*return_value).u1.type_info = php::IS_LONG;
}

/// Rendered inside `phpinfo()` / `php --ri php_all_sys_example`.
unsafe extern "C" fn minfo(_module: *mut php::zend_module_entry) {
    // The detected API number, formatted for the (C-string-only) info table.
    // `php_info_print_table_row` prints synchronously, so a scoped CString is
    // enough -- it need not outlive the call.
    let api = CString::new(host_zend_api().to_string()).unwrap();
    php::php_info_print_table_start();
    php::php_info_print_table_row(2, c"php-all-sys example".as_ptr(), c"enabled".as_ptr());
    php::php_info_print_table_row(2, c"Detected Zend API".as_ptr(), api.as_ptr());
    php::php_info_print_table_end();
}

/// Build the `zend_function_entry` table with the correct per-version stride.
///
/// The first five fields -- `fname@0, handler@8, arg_info@16, num_args@24,
/// flags@28` -- sit at identical offsets in both the 32- and 48-byte layouts,
/// so we allocate a zeroed buffer at the host's stride and write only the three
/// we use. `num_args`/`flags` stay 0, and a fully-zeroed trailing entry is
/// `ZEND_FE_END`. Offsets are naturally aligned, so plain `ptr::write` is fine.
///
/// The buffer is intentionally leaked: the module PHP registers must outlive
/// this call for the whole process lifetime.
unsafe fn build_functions(
    host_api: c_uint,
    arg_info: *const php::zend_internal_arg_info,
) -> *const php::zend_function_entry {
    let fe_size = if host_api >= API_ZEND_FUNCTION_ENTRY_48 {
        FE_SIZE_WIDE
    } else {
        FE_SIZE_NARROW
    };

    // Three entries: two real functions + the ZEND_FE_END terminator.
    let layout = Layout::from_size_align(fe_size * 3, 8).unwrap();
    let buf = alloc_zeroed(layout);
    if buf.is_null() {
        handle_alloc_error(layout);
    }

    let entries: [(&'static CStr, php::zif_handler); 2] =
        [(FN_HELLO, Some(zif_hello)), (FN_ZEND_API, Some(zif_zend_api))];
    for (i, (name, handler)) in entries.iter().enumerate() {
        let base = buf.add(i * fe_size);
        ptr::write(base as *mut *const c_char, name.as_ptr()); // fname@0
        ptr::write(base.add(8) as *mut php::zif_handler, *handler); // handler@8
        ptr::write(base.add(16) as *mut *const php::zend_internal_arg_info, arg_info); // arg_info@16
    }

    buf as *const php::zend_function_entry
}

/// Construct the module description PHP asks for via `get_module()`.
///
/// PHP mutates the module entry after registration (module_number, handle,
/// ...), so it must outlive the call: every piece is leaked to `'static`.
unsafe fn build_module() -> *mut php::zend_module_entry {
    // Learn the running interpreter's exact ABI from one of its own built-in
    // module entries. This is an unreachable safeguard: `standard` and `core`
    // are always registered before any external extension loads. (There is no
    // graceful degradation to offer -- PHP dereferences the returned pointer
    // without a NULL check, so a miss here would crash rather than error; it
    // simply never happens on a real PHP.)
    let Some(host) = host_module() else {
        return ptr::null_mut();
    };

    // A single zeroed arg-info entry is `ZEND_BEGIN_ARG_INFO_EX(0, ...)`: zero
    // required args, no declared return type. Both zero-arg functions share it.
    // (32 bytes and layout-identical across 8.x, so no per-version branch.)
    let arg_info: &'static [php::zend_internal_arg_info; 1] =
        Box::leak(Box::new([core::mem::zeroed()]));

    let functions = build_functions(host.zend_api, arg_info.as_ptr());

    // Zero everything (== STANDARD_MODULE_PROPERTIES) then fill the header.
    let mut m: php::zend_module_entry = core::mem::zeroed();
    // Echo the load-gate fields byte-for-byte from the host so PHP's `zend_api`
    // (==) and `build_id` (strcmp) checks pass on every version; `size`/`zts`/
    // `zend_debug` stay consistent with the interpreter (and cover NTS/ZTS,
    // debug, and any extra build-id suffix for free).
    m.size = host.size;
    m.zend_api = host.zend_api;
    m.zend_debug = host.zend_debug;
    m.zts = host.zts;
    m.build_id = host.build_id;
    // Our own identity and behaviour.
    m.name = EXT_NAME.as_ptr();
    m.functions = functions;
    m.info_func = Some(minfo);
    m.version = EXT_VERSION.as_ptr();

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
