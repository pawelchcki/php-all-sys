# *All PHP versions -sys crate

This crate is meant to facilitate creation of custom PHP extensions in Rust. It provides C bindings for all supported PHP versions.

This crate went through a few iterations, for now its a single crate containing all PHP versions as separate modules - this could change in the future.

In the beginning it also was generating Rust bindings on demand. But currently bindings are meant to be pregenerated - to test if the same crate works on OSX and Windows - without having to vendor OSX and Windows PHP headers.

Its possible we'll need to vendor OSX and Windows PHP includes to fully support these platforms.

## Support

| PHP Version | Linux | Windows | OSX | ZTS Linux | ZTS Windows |
|-------------|-------|---------|-----|-----------|-------------|
| PHP 5.4     |  Yes  |         |     |           |             |
| PHP 5.5     |       |         |     |           |             |
| PHP 5.6     |  Yes  |         |     |    Yes    |             |
| PHP 7.0     |  Yes  |         |     |           |             |
| PHP 7.1     |  Yes  |         |     |           |             |
| PHP 7.2     |  Yes  |         |     |           |             |
| PHP 7.3     |  Yes  |         |     |           |             |
| PHP 7.4     |  Yes  |         |     |           |             |
| PHP 8.0     |  Yes  |         |     |           |             |
| PHP 8.1     |  Yes  |         |     |           |             |
| PHP 8.2     |  Yes  |         |     |           |             |
| PHP 8.3     |  Yes  |         |     |           |             |
| PHP 8.4     |  Yes  |         |     |           |             |
| PHP 8.5     |  Yes  |         |     |           |             |

## Example extension — one `.so` that loads on every PHP 8.x

[`example-ext/`](example-ext) is a complete, loadable PHP extension that is
built **once** into a single `.so` and loads unchanged into every supported
PHP 8.x (8.0–8.5). It exports two userland functions and a `phpinfo()` section:

```php
php_all_sys_hello();     // "Hello from Rust 🦀 — single .so, loaded into PHP with Zend API 20230831"
php_all_sys_zend_api();  // 20230831  (the host's Zend module API, detected at load time)
```

When PHP loads a `.so` it validates two fields of the module entry byte-for-byte
— the Zend module API number (`==`) and the build id (`strcmp`) — so a module
with those values baked in at compile time loads on exactly one PHP minor.
Instead of baking them in, this extension **reads them from the host at load
time**: PHP registers its built-in modules (ext/standard, Core, …) into the
exported `module_registry` before it loads any external extension, and each one
carries the running engine's exact `zend_api`, `build_id`, `zts`, `zend_debug`
and `size`. `get_module()` reads one of them and echoes those fields, so the
load-gate matches on every version — NTS/ZTS/debug suffixes handled for free.

The only ABI shape that actually changed across 8.x is `zend_function_entry`
(32 bytes on 8.0–8.3, 48 bytes on 8.4+, which appended two fields); its stride
is picked at runtime from the detected API number. Every other type the code
touches is byte-identical across 8.x, so a single `php-all-sys` binding is
ABI-correct for all of them and the code never names a concrete version.

Build it once and load it into any PHP 8.x:

```bash
cargo build --release --manifest-path example-ext/Cargo.toml   # no feature flags
php -d extension=target/release/libphp_all_sys_example.so \
    -r 'echo php_all_sys_hello(), "\n";'                       # works on 8.0 … 8.5
```

CI proves the claim: a `build-universal` job compiles the `.so` **once** and
uploads it, then a `load` matrix over PHP 8.0–8.5 downloads that *same* artifact,
loads it into each real PHP, and runs [`example-ext/tests/smoke.php`](example-ext/tests/smoke.php)
— which asserts the extension loaded and that the Zend API it detected is the one
the running PHP actually uses.

## Generating bindings

The `cargo-php-sys-build` crate can be installed and used to generate binding files.

```bash
cargo install cargo-php-sys-build
cargo php-sys-build # to generate files, should be run from Crates root dir
cargo fmt # to format generated files before commiting 
```
