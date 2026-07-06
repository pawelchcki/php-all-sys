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

## Example extension — one source, every PHP 8.x

[`example-ext/`](example-ext) is a complete, loadable PHP extension written
**once** and compiled unchanged against every supported PHP 8.x ABI. It exports
two userland functions and a `phpinfo()` section:

```php
php_all_sys_hello();     // "Hello from Rust 🦀, built against PHP 8.3.32"
php_all_sys_zend_api();  // 20230831  (the Zend module API it was built for)
```

A compiled extension can only match *one* PHP ABI — the Zend module API number
and build id are checked byte-for-byte when PHP loads the `.so`. So this is not
one binary that loads everywhere; it is one **source** that a Cargo feature
targets at a chosen version. Everything version-specific (struct layouts — e.g.
`zend_function_entry` grew from 32 to 48 bytes in 8.4 — constant values, the API
number baked into the build id) comes from `php-all-sys`, so the code never
names a concrete version.

Build it for a specific version and load it:

```bash
# pick exactly one: php80 php81 php82 php83 php84 php85
cargo build --release --manifest-path example-ext/Cargo.toml --features php83
php -d extension=target/release/libphp_all_sys_example.so \
    -r 'echo php_all_sys_hello(), "\n";'
```

CI proves the claim: the `extension` job runs a matrix over PHP 8.0–8.5, and for
each version installs that real PHP, builds the example against the matching
feature, loads the `.so`, and runs [`example-ext/tests/smoke.php`](example-ext/tests/smoke.php)
— which asserts the extension loaded and that it was built for the very PHP
now running it.

## Generating bindings

private crate sys-build can be installed and used to generate binding files

```bash
cargo install --path sys-build
cargo php-sys-build # to generate files, should be run from Crates root dir
cargo fmt # to format generated files before commiting 
```