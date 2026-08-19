#include <assert.h>
#include <stddef.h>
#include <stdint.h>

#include "voice_engine.h"

_Static_assert(offsetof(aivs_engine_info_t, struct_size) == 0U, "struct_size must lead ABI structures");
_Static_assert(offsetof(aivs_engine_info_t, abi_version) == sizeof(uint32_t), "abi_version follows size");
_Static_assert(offsetof(aivs_voice_engine_api_t, struct_size) == 0U, "function table must be versioned");
_Static_assert(
    sizeof(aivs_error_code_t) == sizeof(int32_t),
    "the ABI error code must retain its canonical fixed-width representation"
);
_Static_assert(
    _Generic(&aivs_voice_engine_get_api, aivs_voice_engine_get_api_fn : 1, default : 0),
    "the unique factory symbol must retain its declared function type"
);

int main(void) {
  aivs_voice_engine_api_t api = AIVS_VOICE_ENGINE_API_INIT;
  aivs_mutable_bytes_buffer_t result = AIVS_MUTABLE_BYTES_BUFFER_INIT;

  assert(api.struct_size == sizeof(aivs_voice_engine_api_t));
  assert(api.abi_version == AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION);
  assert(result.written_or_required_bytes == 0U);
  assert(AIVS_ERROR_SUCCESS == 0);
  assert(AIVS_ERROR_UNSUPPORTED_VOICE_ENGINE_ABI == 1001);
  assert(AIVS_ERROR_INVALID_ARGUMENT == 1301);
  assert(AIVS_ERROR_INVALID_STATE == 1302);
  assert(AIVS_ERROR_BUFFER_TOO_SMALL == 1303);

  return 0;
}
