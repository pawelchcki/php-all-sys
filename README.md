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

## Generating bindings

private crate sys-build can be installed and used to generate binding files

```bash
cargo install --path sys-build
cargo php-sys-build # to generate files, should be run from Crates root dir
cargo fmt # to format generated files before commiting 
```