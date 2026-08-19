#ifdef NDEBUG
#undef NDEBUG
#endif

#include <array>
#include <bit>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <limits>
#include <string>
#include <string_view>

#include <voice_engine.h>

#if defined(_WIN32)
#include <windows.h>
#else
#include <dlfcn.h>
#endif

namespace {

class DynamicLibrary {
 public:
  explicit DynamicLibrary(const char* path) {
#if defined(_WIN32)
    const int required = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, nullptr, 0);
    assert(required > 0);
    std::wstring wide(static_cast<std::size_t>(required), L'\0');
    assert(MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide.data(), required) > 0);
    handle_ = LoadLibraryW(wide.c_str());
#else
    handle_ = dlopen(path, RTLD_NOW | RTLD_LOCAL);
#endif
    assert(handle_ != nullptr);
  }

  ~DynamicLibrary() {
#if defined(_WIN32)
    assert(FreeLibrary(handle_) != 0);
#else
    assert(dlclose(handle_) == 0);
#endif
  }

  aivs_voice_engine_get_api_fn factory() const {
#if defined(_WIN32)
    const auto symbol = GetProcAddress(handle_, "aivs_voice_engine_get_api");
#else
    const auto symbol = dlsym(handle_, "aivs_voice_engine_get_api");
#endif
    assert(symbol != nullptr);
    aivs_voice_engine_get_api_fn result{};
    static_assert(sizeof(result) == sizeof(symbol));
    std::memcpy(&result, &symbol, sizeof(result));
    return result;
  }

 private:
#if defined(_WIN32)
  HMODULE handle_{nullptr};
#else
  void* handle_{nullptr};
#endif
};

aivs_bytes_view_t bytes_view(std::string_view value) {
  auto view = aivs_bytes_view_t AIVS_BYTES_VIEW_INIT;
  view.data = value.empty() ? nullptr : reinterpret_cast<const std::uint8_t*>(value.data());
  view.size_bytes = value.size();
  return view;
}

aivs_voice_engine_api_t complete_api(aivs_voice_engine_get_api_fn factory) {
  auto request = aivs_voice_engine_factory_request_t AIVS_VOICE_ENGINE_FACTORY_REQUEST_INIT;
  auto api = aivs_voice_engine_api_t AIVS_VOICE_ENGINE_API_OUTPUT_INIT;
  assert(factory(&request, &api, sizeof(api)) == AIVS_ERROR_SUCCESS);
  assert(aivs_voice_engine_api_v1_is_complete(&api) == AIVS_TRUE);
  return api;
}

aivs_voice_engine_handle_t* initialize(
    const aivs_voice_engine_api_t& api,
    std::string_view configuration = R"({"work_iterations":0})") {
  auto request = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
  request.configuration_utf8 = bytes_view(configuration);
  auto result = aivs_initialize_result_t AIVS_INITIALIZE_RESULT_INIT;
  assert(api.initialize(&request, &result) == AIVS_ERROR_SUCCESS);
  assert(result.engine != nullptr);
  return result.engine;
}

void shutdown(const aivs_voice_engine_api_t& api, aivs_voice_engine_handle_t* engine) {
  aivs_shutdown_request_t request{
      sizeof(aivs_shutdown_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  assert(api.shutdown(engine, &request) == AIVS_ERROR_SUCCESS);
}

aivs_load_model_request_t model_request(std::string_view id = "mock-v1") {
  return {
      sizeof(aivs_load_model_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      bytes_view(id),
      AIVS_BYTES_VIEW_INIT,
      {0U, 0U},
  };
}

aivs_prepare_stream_request_t prepare_request() {
  auto request = aivs_prepare_stream_request_t AIVS_PREPARE_STREAM_REQUEST_INIT;
  request.sample_rate_hz = 48'000U;
  request.channel_count = 2U;
  request.maximum_frame_count = 4U;
  request.stream_id = 1U;
  return request;
}

void assert_full_api_cleared(const aivs_voice_engine_api_t& api) {
  assert(api.struct_size == 0U);
  assert(api.abi_version == 0U);
  assert(api.initialize == nullptr);
  assert(api.shutdown == nullptr);
  assert(api.get_engine_info == nullptr);
  assert(api.load_model == nullptr);
  assert(api.prepare_stream == nullptr);
  assert(api.process_audio == nullptr);
  assert(api.reset == nullptr);
  assert(api.get_metrics == nullptr);
  for (const auto reserved : api.reserved) {
    assert(reserved == 0U);
  }
}

void factory_failure_matrix_is_bounded_and_negotiates_v1(aivs_voice_engine_get_api_fn factory) {
  auto valid = aivs_voice_engine_factory_request_t AIVS_VOICE_ENGINE_FACTORY_REQUEST_INIT;
  assert(factory(&valid, nullptr, AIVS_VOICE_ENGINE_API_V1_SIZE) ==
         AIVS_ERROR_INVALID_ARGUMENT);

  auto full = aivs_voice_engine_api_t AIVS_VOICE_ENGINE_API_OUTPUT_INIT;
  std::memset(&full, 0xA5, sizeof(full));
  assert(factory(nullptr, &full, sizeof(full)) == AIVS_ERROR_INVALID_ARGUMENT);
  assert_full_api_cleared(full);

  const std::array malformed_requests{
      [&] {
        auto value = valid;
        value.struct_size = sizeof(value) - 1U;
        return value;
      }(),
      [&] {
        auto value = valid;
        value.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = valid;
        value.reserved[0] = 1U;
        return value;
      }(),
      [&] {
        auto value = valid;
        value.minimum_abi_version = 0U;
        return value;
      }(),
      [&] {
        auto value = valid;
        value.minimum_abi_version = 2U;
        value.maximum_abi_version = 1U;
        return value;
      }(),
  };
  for (const auto& malformed : malformed_requests) {
    std::memset(&full, 0xA5, sizeof(full));
    assert(factory(&malformed, &full, sizeof(full)) == AIVS_ERROR_INVALID_ARGUMENT);
    assert_full_api_cleared(full);
  }

  auto no_overlap = valid;
  no_overlap.minimum_abi_version = 2U;
  no_overlap.maximum_abi_version = 2U;
  std::memset(&full, 0xA5, sizeof(full));
  assert(factory(&no_overlap, &full, sizeof(full)) ==
         AIVS_ERROR_UNSUPPORTED_VOICE_ENGINE_ABI);
  assert_full_api_cleared(full);

  constexpr std::array<std::uint32_t, 4> capacities{
      0U,
      static_cast<std::uint32_t>(sizeof(std::uint32_t)),
      static_cast<std::uint32_t>(offsetof(aivs_voice_engine_api_t, process_audio)),
      AIVS_VOICE_ENGINE_API_V1_SIZE - 1U,
  };
  alignas(aivs_voice_engine_api_t)
      std::array<std::uint8_t, sizeof(aivs_voice_engine_api_t) + 16U> storage{};
  for (const auto capacity : capacities) {
    storage.fill(0xA5U);
    auto* output = reinterpret_cast<aivs_voice_engine_api_t*>(storage.data());
    assert(factory(&valid, output, capacity) == AIVS_ERROR_BUFFER_TOO_SMALL);
    for (std::size_t index = 0U; index < storage.size(); ++index) {
      assert(storage[index] == (index < capacity ? 0U : 0xA5U));
    }
  }

  const auto api = complete_api(factory);
  assert(api.struct_size == AIVS_VOICE_ENGINE_API_V1_SIZE);
  assert(api.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);

  auto future_compatible = valid;
  future_compatible.maximum_abi_version = AIVS_VOICE_ENGINE_ABI_V1_VERSION + 1U;
  auto negotiated = aivs_voice_engine_api_t AIVS_VOICE_ENGINE_API_OUTPUT_INIT;
  assert(factory(&future_compatible, &negotiated, sizeof(negotiated)) == AIVS_ERROR_SUCCESS);
  assert(negotiated.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(aivs_voice_engine_api_v1_is_complete(&negotiated) == AIVS_TRUE);
}

void initialize_and_shutdown_validate_prefix_version_reserved_and_retry(
    const aivs_voice_engine_api_t& api) {
  const std::array invalid_initializers{
      [&] {
        auto value = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
        value.configuration_utf8 = bytes_view(R"({"work_iterations":0})");
        value.struct_size = sizeof(value) - 1U;
        return value;
      }(),
      [&] {
        auto value = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
        value.configuration_utf8 = bytes_view(R"({"work_iterations":0})");
        value.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
        value.configuration_utf8 = bytes_view(R"({"work_iterations":0})");
        value.reserved[0] = 1U;
        return value;
      }(),
      [&] {
        auto value = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
        value.configuration_utf8 = bytes_view(R"({"work_iterations":0})");
        value.configuration_utf8.struct_size = sizeof(value.configuration_utf8) - 1U;
        return value;
      }(),
      [&] {
        auto value = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
        value.configuration_utf8 = bytes_view(R"({"work_iterations":0})");
        value.configuration_utf8.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
        value.configuration_utf8 = bytes_view(R"({"work_iterations":0})");
        value.configuration_utf8.reserved[0] = 1U;
        return value;
      }(),
  };
  for (const auto& request : invalid_initializers) {
    auto result = aivs_initialize_result_t AIVS_INITIALIZE_RESULT_INIT;
    result.engine = reinterpret_cast<aivs_voice_engine_handle_t*>(static_cast<std::uintptr_t>(1U));
    assert(api.initialize(&request, &result) == AIVS_ERROR_INVALID_ARGUMENT);
    assert(result.engine == nullptr);
  }
  auto valid_initialize = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
  valid_initialize.configuration_utf8 = bytes_view(R"({"work_iterations":0})");
  const std::array invalid_results{
      [&] {
        auto value = aivs_initialize_result_t AIVS_INITIALIZE_RESULT_INIT;
        value.struct_size = sizeof(value) - 1U;
        return value;
      }(),
      [&] {
        auto value = aivs_initialize_result_t AIVS_INITIALIZE_RESULT_INIT;
        value.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = aivs_initialize_result_t AIVS_INITIALIZE_RESULT_INIT;
        value.reserved[0] = 1U;
        return value;
      }(),
  };
  for (auto result : invalid_results) {
    result.engine = reinterpret_cast<aivs_voice_engine_handle_t*>(static_cast<std::uintptr_t>(1U));
    assert(api.initialize(&valid_initialize, &result) == AIVS_ERROR_INVALID_ARGUMENT);
    assert(result.engine == nullptr);
  }
  for (const auto malformed : {
           std::string_view("{}"),
           std::string_view(R"({"work_iterations":01})"),
           std::string_view(R"({"work_iterations":1000001})"),
           std::string_view(R"({"work_iterations":0,"unknown":1})"),
           std::string_view(R"({"work_iterations":0,"initial_generation":1})"),
           std::string_view(R"({"work_iterations":0,"initial_generation":01})"),
           std::string_view(R"({"work_iterations":0,"initial_generation":18446744073709551616})"),
       }) {
    auto request = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
    request.configuration_utf8 = bytes_view(malformed);
    auto result = aivs_initialize_result_t AIVS_INITIALIZE_RESULT_INIT;
    assert(api.initialize(&request, &result) == AIVS_ERROR_INVALID_ARGUMENT);
    assert(result.engine == nullptr);
  }

  auto* engine = initialize(api);
  const std::array invalid_shutdowns{
      aivs_shutdown_request_t{
          sizeof(aivs_shutdown_request_t) - 1U,
          AIVS_VOICE_ENGINE_ABI_V1_VERSION,
          {0U, 0U}},
      aivs_shutdown_request_t{sizeof(aivs_shutdown_request_t), 2U, {0U, 0U}},
      aivs_shutdown_request_t{
          sizeof(aivs_shutdown_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {1U, 0U}},
  };
  for (const auto& request : invalid_shutdowns) {
    assert(api.shutdown(engine, &request) == AIVS_ERROR_INVALID_ARGUMENT);
  }
  aivs_get_metrics_request_t metrics_request{
      sizeof(aivs_get_metrics_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  aivs_engine_metrics_t metrics{
      sizeof(aivs_engine_metrics_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, 0U, 0U, 0U, 0U, {0U, 0U}};
  assert(api.get_metrics(engine, &metrics_request, &metrics) == AIVS_ERROR_SUCCESS);
  shutdown(api, engine);
}

void info_rejects_prefix_version_reserved_overlap_and_normalizes_capacity(
    const aivs_voice_engine_api_t& api) {
  auto* engine = initialize(api);
  std::array<std::uint8_t, 64> first{};
  std::array<std::uint8_t, 32> second{};

  auto invalid = aivs_engine_info_t AIVS_ENGINE_INFO_INIT;
  invalid.engine_name_utf8.data = first.data();
  invalid.engine_name_utf8.capacity_bytes = first.size();
  invalid.engine_version_utf8.data = second.data();
  invalid.engine_version_utf8.capacity_bytes = second.size();
  invalid.engine_name_utf8.written_or_required_bytes = 99U;
  invalid.engine_version_utf8.written_or_required_bytes = 99U;
  invalid.abi_version = 2U;
  assert(api.get_engine_info(engine, &invalid) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(invalid.engine_name_utf8.written_or_required_bytes == 0U);
  assert(invalid.engine_version_utf8.written_or_required_bytes == 0U);
  invalid.abi_version = AIVS_VOICE_ENGINE_ABI_V1_VERSION;
  invalid.reserved[0] = 1U;
  invalid.engine_name_utf8.written_or_required_bytes = 99U;
  invalid.engine_version_utf8.written_or_required_bytes = 99U;
  assert(api.get_engine_info(engine, &invalid) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(invalid.engine_name_utf8.written_or_required_bytes == 0U);
  assert(invalid.engine_version_utf8.written_or_required_bytes == 0U);

  invalid.reserved[0] = 0U;
  invalid.struct_size = sizeof(invalid) - 1U;
  invalid.engine_name_utf8.written_or_required_bytes = 99U;
  invalid.engine_version_utf8.written_or_required_bytes = 99U;
  assert(api.get_engine_info(engine, &invalid) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(invalid.engine_name_utf8.written_or_required_bytes == 0U);
  assert(invalid.engine_version_utf8.written_or_required_bytes == 0U);
  invalid.struct_size = sizeof(invalid);

  const std::array invalid_nested_infos{
      [&] {
        auto value = invalid;
        value.engine_name_utf8.struct_size = sizeof(value.engine_name_utf8) - 1U;
        return value;
      }(),
      [&] {
        auto value = invalid;
        value.engine_name_utf8.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = invalid;
        value.engine_name_utf8.reserved[0] = 1U;
        return value;
      }(),
  };
  for (auto nested : invalid_nested_infos) {
    nested.engine_name_utf8.written_or_required_bytes = 99U;
    nested.engine_version_utf8.written_or_required_bytes = 99U;
    assert(api.get_engine_info(engine, &nested) == AIVS_ERROR_INVALID_ARGUMENT);
    assert(nested.engine_name_utf8.written_or_required_bytes == 0U);
    assert(nested.engine_version_utf8.written_or_required_bytes == 0U);
  }

  auto exact_overlap = aivs_engine_info_t AIVS_ENGINE_INFO_INIT;
  exact_overlap.engine_name_utf8.data = first.data();
  exact_overlap.engine_name_utf8.capacity_bytes = first.size();
  exact_overlap.engine_version_utf8.data = first.data();
  exact_overlap.engine_version_utf8.capacity_bytes = first.size();
  assert(api.get_engine_info(engine, &exact_overlap) == AIVS_ERROR_INVALID_ARGUMENT);

  auto partial_overlap = exact_overlap;
  partial_overlap.engine_version_utf8.data = first.data() + 20U;
  partial_overlap.engine_version_utf8.capacity_bytes = 5U;
  assert(api.get_engine_info(engine, &partial_overlap) == AIVS_ERROR_INVALID_ARGUMENT);

  auto overlap_before_capacity = exact_overlap;
  overlap_before_capacity.engine_name_utf8.capacity_bytes = 1U;
  overlap_before_capacity.engine_version_utf8.capacity_bytes = 1U;
  assert(api.get_engine_info(engine, &overlap_before_capacity) == AIVS_ERROR_INVALID_ARGUMENT);

  auto wrapped_range = aivs_engine_info_t AIVS_ENGINE_INFO_INIT;
  wrapped_range.engine_name_utf8.data = reinterpret_cast<std::uint8_t*>(
      UINTPTR_MAX - static_cast<std::uintptr_t>(7U));
  wrapped_range.engine_name_utf8.capacity_bytes = 1U;
  assert(api.get_engine_info(engine, &wrapped_range) == AIVS_ERROR_INVALID_ARGUMENT);

  auto short_info = aivs_engine_info_t AIVS_ENGINE_INFO_INIT;
  short_info.engine_name_utf8.data = first.data();
  short_info.engine_name_utf8.capacity_bytes = 1U;
  short_info.engine_version_utf8.data = second.data();
  short_info.engine_version_utf8.capacity_bytes = 1U;
  assert(api.get_engine_info(engine, &short_info) == AIVS_ERROR_BUFFER_TOO_SMALL);
  assert(short_info.engine_name_utf8.written_or_required_bytes == 21U);
  assert(short_info.engine_version_utf8.written_or_required_bytes == 5U);
  shutdown(api, engine);
}

void model_prepare_reset_and_normal_generation_preserve_state(
    const aivs_voice_engine_api_t& api) {
  auto* engine = initialize(api);
  auto wrong = model_request("wrong");
  assert(api.load_model(engine, &wrong) == AIVS_ERROR_INVALID_ARGUMENT);
  auto nonempty = model_request();
  const std::array<std::uint8_t, 1> data{0x01U};
  nonempty.model_data.data = data.data();
  nonempty.model_data.size_bytes = data.size();
  assert(api.load_model(engine, &nonempty) == AIVS_ERROR_INVALID_ARGUMENT);

  auto prepare = prepare_request();
  auto prepared = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
  prepare.format = 99U;
  assert(api.prepare_stream(engine, &prepare, &prepared) == AIVS_ERROR_INVALID_ARGUMENT);
  prepare.format = AIVS_PCM_FORMAT_FLOAT32;
  assert(api.prepare_stream(engine, &prepare, &prepared) == AIVS_ERROR_INVALID_STATE);
  auto reset_request = aivs_reset_request_t{
      sizeof(aivs_reset_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_RESET_REASON_CALLER_REQUEST,
      0U,
      {0U, 0U},
  };
  auto reset_result = aivs_reset_result_t AIVS_RESET_RESULT_INIT;
  reset_request.reason = 99U;
  assert(api.reset(engine, &reset_request, &reset_result) == AIVS_ERROR_INVALID_ARGUMENT);
  reset_request.reason = AIVS_RESET_REASON_CALLER_REQUEST;
  assert(api.reset(engine, &reset_request, &reset_result) == AIVS_ERROR_INVALID_STATE);

  auto load = model_request();
  const std::array invalid_loads{
      [&] {
        auto value = load;
        value.struct_size = sizeof(value) - 1U;
        return value;
      }(),
      [&] {
        auto value = load;
        value.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = load;
        value.reserved[0] = 1U;
        return value;
      }(),
      [&] {
        auto value = load;
        value.model_id_utf8.struct_size = sizeof(value.model_id_utf8) - 1U;
        return value;
      }(),
      [&] {
        auto value = load;
        value.model_id_utf8.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = load;
        value.model_data.reserved[0] = 1U;
        return value;
      }(),
  };
  for (const auto& invalid : invalid_loads) {
    assert(api.load_model(engine, &invalid) == AIVS_ERROR_INVALID_ARGUMENT);
  }
  assert(api.load_model(engine, &load) == AIVS_ERROR_SUCCESS);

  const std::array invalid_prepares{
      [&] {
        auto value = prepare;
        value.struct_size = sizeof(value) - 1U;
        return value;
      }(),
      [&] {
        auto value = prepare;
        value.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = prepare;
        value.reserved[0] = 1U;
        return value;
      }(),
  };
  for (const auto& invalid : invalid_prepares) {
    prepared.algorithmic_latency_frames = 99U;
    prepared.stream_generation = 99U;
    assert(api.prepare_stream(engine, &invalid, &prepared) == AIVS_ERROR_INVALID_ARGUMENT);
    assert(prepared.algorithmic_latency_frames == 0U);
    assert(prepared.stream_generation == 0U);
  }
  const std::array invalid_prepare_results{
      [&] {
        auto value = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
        value.struct_size = sizeof(value) - 1U;
        return value;
      }(),
      [&] {
        auto value = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
        value.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
        value.reserved[0] = 1U;
        return value;
      }(),
  };
  for (auto invalid_result : invalid_prepare_results) {
    invalid_result.algorithmic_latency_frames = 99U;
    invalid_result.stream_generation = 99U;
    assert(api.prepare_stream(engine, &prepare, &invalid_result) == AIVS_ERROR_INVALID_ARGUMENT);
    assert(invalid_result.algorithmic_latency_frames == 0U);
    assert(invalid_result.stream_generation == 0U);
  }
  assert(api.prepare_stream(engine, &prepare, &prepared) == AIVS_ERROR_SUCCESS);
  assert(prepared.stream_generation == 1U);

  const std::array invalid_resets{
      [&] {
        auto value = reset_request;
        value.struct_size = sizeof(value) - 1U;
        return value;
      }(),
      [&] {
        auto value = reset_request;
        value.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = reset_request;
        value.reserved[0] = 1U;
        return value;
      }(),
  };
  for (const auto& invalid : invalid_resets) {
    reset_result.stream_generation = 99U;
    assert(api.reset(engine, &invalid, &reset_result) == AIVS_ERROR_INVALID_ARGUMENT);
    assert(reset_result.stream_generation == 0U);
  }
  const std::array invalid_reset_results{
      [&] {
        auto value = aivs_reset_result_t AIVS_RESET_RESULT_INIT;
        value.struct_size = sizeof(value) - 1U;
        return value;
      }(),
      [&] {
        auto value = aivs_reset_result_t AIVS_RESET_RESULT_INIT;
        value.abi_version = 2U;
        return value;
      }(),
      [&] {
        auto value = aivs_reset_result_t AIVS_RESET_RESULT_INIT;
        value.reserved[0] = 1U;
        return value;
      }(),
  };
  for (auto invalid_result : invalid_reset_results) {
    invalid_result.stream_generation = 99U;
    assert(api.reset(engine, &reset_request, &invalid_result) == AIVS_ERROR_INVALID_ARGUMENT);
    assert(invalid_result.stream_generation == 0U);
  }
  assert(api.reset(engine, &reset_request, &reset_result) == AIVS_ERROR_SUCCESS);
  assert(reset_result.stream_generation == 2U);
  assert(api.reset(engine, &reset_request, &reset_result) == AIVS_ERROR_INVALID_STATE);
  assert(api.prepare_stream(engine, &prepare, &prepared) == AIVS_ERROR_SUCCESS);
  assert(prepared.stream_generation == 3U);
  shutdown(api, engine);
}

void generation_exhaustion_is_bounded(const aivs_voice_engine_api_t& api) {
  auto prepare = prepare_request();
  auto prepared = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
  auto reset_request = aivs_reset_request_t{
      sizeof(aivs_reset_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_RESET_REASON_CALLER_REQUEST,
      0U,
      {0U, 0U},
  };
  auto reset_result = aivs_reset_result_t AIVS_RESET_RESULT_INIT;
  auto* exhausted = initialize(
      api,
      R"({"work_iterations":0,"initial_generation":18446744073709551614})");
  auto exhausted_load = model_request();
  assert(api.load_model(exhausted, &exhausted_load) == AIVS_ERROR_SUCCESS);
  assert(api.prepare_stream(exhausted, &prepare, &prepared) == AIVS_ERROR_SUCCESS);
  assert(prepared.stream_generation == UINT64_MAX);
  reset_result.stream_generation = 99U;
  assert(api.reset(exhausted, &reset_request, &reset_result) == AIVS_ERROR_INVALID_STATE);
  assert(reset_result.stream_generation == 0U);

  aivs_process_audio_request_t generation_process{
      sizeof(aivs_process_audio_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_PCM_BUFFER_INIT,
      UINT64_MAX,
      {0U, 0U},
  };
  generation_process.input.sample_rate_hz = 48'000U;
  generation_process.input.channel_count = 2U;
  generation_process.input.frame_count = 0U;
  auto generation_result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  assert(api.process_audio(exhausted, &generation_process, &generation_result) ==
         AIVS_ERROR_SUCCESS);
  shutdown(api, exhausted);

  auto* prepare_exhausted = initialize(
      api, R"({"work_iterations":0,"initial_generation":18446744073709551615})");
  auto prepare_exhausted_load = model_request();
  assert(api.load_model(prepare_exhausted, &prepare_exhausted_load) == AIVS_ERROR_SUCCESS);
  prepared.stream_generation = 99U;
  assert(api.prepare_stream(prepare_exhausted, &prepare, &prepared) == AIVS_ERROR_INVALID_STATE);
  assert(prepared.stream_generation == 0U);
  shutdown(api, prepare_exhausted);
}

void process_and_metrics_validate_nested_contract_and_failure_atomicity(
    const aivs_voice_engine_api_t& api) {
  auto* engine = initialize(api);
  aivs_process_audio_request_t state_precedence{
      sizeof(aivs_process_audio_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_PCM_BUFFER_INIT,
      0U,
      {0U, 0U},
  };
  state_precedence.input.sample_rate_hz = 48'000U;
  state_precedence.input.channel_count = 2U;
  state_precedence.input.format = 99U;
  auto state_result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  assert(api.process_audio(engine, &state_precedence, &state_result) ==
         AIVS_ERROR_INVALID_ARGUMENT);
  state_precedence.input.format = AIVS_PCM_FORMAT_FLOAT32;
  assert(api.process_audio(engine, &state_precedence, &state_result) == AIVS_ERROR_INVALID_STATE);

  auto load = model_request();
  assert(api.load_model(engine, &load) == AIVS_ERROR_SUCCESS);
  auto prepare = prepare_request();
  auto prepared = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
  assert(api.prepare_stream(engine, &prepare, &prepared) == AIVS_ERROR_SUCCESS);

  std::array<float, 8> input{};
  std::array<float, 8> output{};
  for (std::size_t index = 0U; index < input.size(); ++index) {
    input[index] = std::bit_cast<float>(UINT32_C(0x3E800000) + static_cast<std::uint32_t>(index));
  }
  aivs_process_audio_request_t process{
      sizeof(aivs_process_audio_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_PCM_BUFFER_INIT,
      prepared.stream_generation,
      {0U, 0U},
  };
  process.input.samples = input.data();
  process.input.sample_rate_hz = 48'000U;
  process.input.channel_count = 2U;
  process.input.frame_count = 4U;
  auto result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  result.output.samples = output.data();
  result.output.frame_capacity = 4U;

  auto invalid_process = process;
  invalid_process.struct_size = sizeof(invalid_process) - 1U;
  assert(api.process_audio(engine, &invalid_process, &result) == AIVS_ERROR_INVALID_ARGUMENT);
  invalid_process = process;
  invalid_process.abi_version = 2U;
  assert(api.process_audio(engine, &invalid_process, &result) == AIVS_ERROR_INVALID_ARGUMENT);
  invalid_process = process;
  invalid_process.reserved[0] = 1U;
  assert(api.process_audio(engine, &invalid_process, &result) == AIVS_ERROR_INVALID_ARGUMENT);
  invalid_process = process;
  invalid_process.input.struct_size = sizeof(invalid_process.input) - 1U;
  assert(api.process_audio(engine, &invalid_process, &result) == AIVS_ERROR_INVALID_ARGUMENT);
  invalid_process = process;
  invalid_process.input.abi_version = 2U;
  assert(api.process_audio(engine, &invalid_process, &result) == AIVS_ERROR_INVALID_ARGUMENT);
  invalid_process = process;
  invalid_process.input.reserved[0] = 1U;
  assert(api.process_audio(engine, &invalid_process, &result) == AIVS_ERROR_INVALID_ARGUMENT);

  auto invalid_result = result;
  invalid_result.abi_version = 2U;
  invalid_result.processed_frame_count = 99U;
  invalid_result.stream_generation = 99U;
  assert(api.process_audio(engine, &process, &invalid_result) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(invalid_result.processed_frame_count == 0U);
  assert(invalid_result.stream_generation == 0U);
  invalid_result = result;
  invalid_result.output.abi_version = 2U;
  assert(api.process_audio(engine, &process, &invalid_result) == AIVS_ERROR_INVALID_ARGUMENT);
  invalid_result = result;
  invalid_result.output.reserved[0] = 1U;
  assert(api.process_audio(engine, &process, &invalid_result) == AIVS_ERROR_INVALID_ARGUMENT);
  invalid_result = result;
  invalid_result.struct_size = sizeof(invalid_result) - 1U;
  invalid_result.processed_frame_count = 99U;
  invalid_result.stream_generation = 99U;
  assert(api.process_audio(engine, &process, &invalid_result) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(invalid_result.processed_frame_count == 0U);
  assert(invalid_result.stream_generation == 0U);
  invalid_result = result;
  invalid_result.reserved[0] = 1U;
  assert(api.process_audio(engine, &process, &invalid_result) == AIVS_ERROR_INVALID_ARGUMENT);
  invalid_result = result;
  invalid_result.output.struct_size = sizeof(invalid_result.output) - 1U;
  assert(api.process_audio(engine, &process, &invalid_result) == AIVS_ERROR_INVALID_ARGUMENT);

  invalid_process = process;
  invalid_process.input.samples = reinterpret_cast<const float*>(
      UINTPTR_MAX - static_cast<std::uintptr_t>(7U));
  assert(api.process_audio(engine, &invalid_process, &result) == AIVS_ERROR_INVALID_ARGUMENT);

  std::array<float, 9> overlap{};
  invalid_process = process;
  invalid_process.input.samples = overlap.data();
  invalid_result = result;
  invalid_result.output.samples = overlap.data() + 1U;
  assert(api.process_audio(engine, &invalid_process, &invalid_result) == AIVS_ERROR_INVALID_ARGUMENT);

  assert(api.process_audio(engine, &process, &result) == AIVS_ERROR_SUCCESS);
  auto in_place = input;
  process.input.samples = in_place.data();
  result.output.samples = in_place.data();
  assert(api.process_audio(engine, &process, &result) == AIVS_ERROR_SUCCESS);
  process.input.samples = nullptr;
  process.input.frame_count = 0U;
  result.output.samples = nullptr;
  result.output.frame_capacity = 0U;
  assert(api.process_audio(engine, &process, &result) == AIVS_ERROR_SUCCESS);

  aivs_get_metrics_request_t metrics_request{
      sizeof(aivs_get_metrics_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {1U, 0U}};
  aivs_engine_metrics_t metrics{
      sizeof(aivs_engine_metrics_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      99U,
      99U,
      99U,
      99U,
      {0U, 0U},
  };
  assert(api.get_metrics(engine, &metrics_request, &metrics) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(metrics.process_call_count == 0U);
  assert(metrics.input_frame_count == 0U);
  assert(metrics.output_frame_count == 0U);
  assert(metrics.process_error_count == 0U);
  metrics_request.struct_size = sizeof(metrics_request) - 1U;
  metrics.process_call_count = 99U;
  assert(api.get_metrics(engine, &metrics_request, &metrics) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(metrics.process_call_count == 0U);
  metrics_request.struct_size = sizeof(metrics_request);
  metrics_request.abi_version = 2U;
  metrics.process_call_count = 99U;
  assert(api.get_metrics(engine, &metrics_request, &metrics) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(metrics.process_call_count == 0U);
  metrics_request.abi_version = AIVS_VOICE_ENGINE_ABI_V1_VERSION;
  metrics_request.reserved[0] = 0U;
  metrics.struct_size = sizeof(metrics) - 1U;
  metrics.process_call_count = 99U;
  assert(api.get_metrics(engine, &metrics_request, &metrics) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(metrics.process_call_count == 0U);
  metrics.struct_size = sizeof(metrics);
  metrics.abi_version = 2U;
  metrics.process_call_count = 99U;
  assert(api.get_metrics(engine, &metrics_request, &metrics) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(metrics.process_call_count == 0U);
  metrics.abi_version = AIVS_VOICE_ENGINE_ABI_V1_VERSION;
  metrics.reserved[0] = 1U;
  metrics.process_call_count = 99U;
  assert(api.get_metrics(engine, &metrics_request, &metrics) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(metrics.process_call_count == 0U);
  metrics.reserved[0] = 0U;
  assert(api.get_metrics(engine, &metrics_request, &metrics) == AIVS_ERROR_SUCCESS);
  assert(metrics.process_call_count == 3U);
  assert(metrics.input_frame_count == 8U);
  assert(metrics.output_frame_count == 8U);
  assert(metrics.process_error_count == 16U);
  shutdown(api, engine);
}

}  // namespace

int main(int argc, char** argv) {
  assert(argc == 3);
  {
    DynamicLibrary production_library(argv[1]);
    const auto production_factory = production_library.factory();
    factory_failure_matrix_is_bounded_and_negotiates_v1(production_factory);
    const auto production_api = complete_api(production_factory);
    initialize_and_shutdown_validate_prefix_version_reserved_and_retry(production_api);
    info_rejects_prefix_version_reserved_overlap_and_normalizes_capacity(production_api);
    model_prepare_reset_and_normal_generation_preserve_state(production_api);
    process_and_metrics_validate_nested_contract_and_failure_atomicity(production_api);
  }
  {
    DynamicLibrary exhaustion_test_library(argv[2]);
    generation_exhaustion_is_bounded(complete_api(exhaustion_test_library.factory()));
  }
}
