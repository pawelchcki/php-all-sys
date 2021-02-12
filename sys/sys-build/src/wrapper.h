#pragma once 
#include "php.h"
#include "php_ini.h"
#include "ext/standard/info.h"
#include "zend_extensions.h"

// workaround for bindgen not exporting ZEND_MODULE_BUILD_ID correctly
static const char *ZEND_MODULE_BUILD_ID_ = ZEND_MODULE_BUILD_ID;
