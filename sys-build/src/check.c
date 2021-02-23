#include "wrapper.h"

#if DEBUG_ZEND
CHECK_DEBUG_ZEND_TRUE;
#else
CHECK_DEBUG_ZEND_FALSE;
#endif

#if USING_ZTS
CHECK_USING_ZTS_TRUE;
#else
CHECK_USING_ZTS_FALSE;
#endif

//TODO: add more checks
//TODO: consider removing as currently its not used