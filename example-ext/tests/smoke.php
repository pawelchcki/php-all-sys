<?php
// Smoke test for the php-all-sys example extension.
//
// Run as:  php -d extension=/abs/path/libphp_all_sys_example.so smoke.php
//
// It proves the extension we compiled against a specific PHP 8.x ABI actually
// loaded into *this* PHP and that its functions work. The most important check
// is implicit: PHP only reaches this script if it accepted the module's Zend
// API number and build id at load time, i.e. the ABI matched exactly.

declare(strict_types=1);

function fail(string $msg): never {
    fwrite(STDERR, "FAIL: {$msg}\n");
    exit(1);
}

if (!extension_loaded('php_all_sys_example')) {
    fail('extension php_all_sys_example is not loaded');
}

foreach (['php_all_sys_hello', 'php_all_sys_zend_api'] as $fn) {
    if (!function_exists($fn)) {
        fail("function {$fn}() was not registered");
    }
}

$hello = php_all_sys_hello();
echo $hello, "\n";
if (!is_string($hello) || strncmp($hello, 'Hello from Rust', 15) !== 0) {
    fail("unexpected return from php_all_sys_hello(): " . var_export($hello, true));
}

// The extension was compiled against one PHP version; assert it is the very
// version now running it (major.minor), which is the whole point of the crate.
[$maj, $min] = explode('.', PHP_VERSION);
$want = "PHP {$maj}.{$min}";
if (strpos($hello, $want) === false) {
    fail("greeting '{$hello}' does not mention the running {$want}");
}

$api = php_all_sys_zend_api();
echo "zend_api={$api}\n";
if (!is_int($api) || $api < 20000000) {
    fail("php_all_sys_zend_api() returned a bogus value: " . var_export($api, true));
}

echo "OK ({$want}, Zend API {$api})\n";
