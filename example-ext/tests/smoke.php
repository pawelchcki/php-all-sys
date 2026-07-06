<?php
// Smoke test for the php-all-sys example extension.
//
// Run as:  php -d extension=/abs/path/libphp_all_sys_example.so smoke.php
//
// The extension is a *single* .so that loads into any PHP 8.x. The strongest
// proof of that is implicit: PHP only reaches this script if it accepted the
// module's Zend API number and build id at load time, i.e. the values the
// extension echoed back from the host matched this interpreter exactly. On top
// of that we assert the runtime-detected API number is the one this PHP really
// uses -- so the same binary passing on every version proves portability.

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

// The extension detects the host's Zend module API number at load time and
// reports it. Assert it matches the API of the PHP actually running us.
$api_by_minor = [
    '8.0' => 20200930,
    '8.1' => 20210902,
    '8.2' => 20220829,
    '8.3' => 20230831,
    '8.4' => 20240924,
    '8.5' => 20250925,
];
[$maj, $min] = explode('.', PHP_VERSION);
$minor = "{$maj}.{$min}";
if (!isset($api_by_minor[$minor])) {
    fail("smoke test has no expected Zend API for PHP {$minor}; add it to the map");
}
$want_api = $api_by_minor[$minor];

$api = php_all_sys_zend_api();
echo "zend_api={$api}\n";
if (!is_int($api)) {
    fail("php_all_sys_zend_api() did not return an int: " . var_export($api, true));
}
if ($api !== $want_api) {
    fail("detected Zend API {$api} != expected {$want_api} for PHP {$minor}");
}

echo "OK (PHP {$minor}, detected Zend API {$api})\n";
