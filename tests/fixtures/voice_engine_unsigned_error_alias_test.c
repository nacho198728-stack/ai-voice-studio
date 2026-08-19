#define aivs_error_code_t aivs_test_original_error_code_t
#include <ai_voice_contracts/generated_contracts_c.h>
#undef aivs_error_code_t

typedef uint32_t aivs_error_code_t;

#include "../contract/voice_engine_c_abi_c_test.c"
