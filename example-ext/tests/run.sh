#!/usr/bin/env bash
# Load the built extension into a real PHP and run the smoke test.
#
# Usage: run.sh <path-to.so> [php-binary]
set -euo pipefail

SO="${1:?usage: run.sh <path-to .so> [php-binary]}"
PHP="${2:-php}"
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

SO="$(cd "$(dirname "$SO")" && pwd)/$(basename "$SO")" # absolutise

echo "== $($PHP -v | head -n1) =="
echo "== php --ri php_all_sys_example =="
"$PHP" -d extension="$SO" --ri php_all_sys_example
echo "== smoke.php =="
"$PHP" -d extension="$SO" "$DIR/smoke.php"
