#ifdef NDEBUG
#undef NDEBUG
#endif

#include <algorithm>
#include <cassert>
#include <chrono>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <iostream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <ai_voice_runtime/logging.hpp>

#include "jsonl_test_support.hpp"

namespace test_support {

[[noreturn]] void assertion_failed(const char* expression, int line) {
  throw std::runtime_error(
      "Runtime logging assertion failed at line " + std::to_string(line) + ": " + expression);
}

}  // namespace test_support

#undef assert
#define assert(expression) \
  ((expression) ? static_cast<void>(0) : test_support::assertion_failed(#expression, __LINE__))

namespace runtime = ai_voice::runtime;

namespace {

std::filesystem::path unique_directory(std::string_view name) {
  const auto nonce = std::chrono::steady_clock::now().time_since_epoch().count();
  return std::filesystem::temp_directory_path() /
         ("aivs-runtime-logging-" + std::string(name) + "-" + std::to_string(nonce));
}

std::string read_file(const std::filesystem::path& path) {
  std::ifstream input(path, std::ios::binary);
  assert(input.good());
  return {std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>()};
}

runtime::LoggingPolicy logging_policy(bool debug_enabled, runtime::LogLevel level) {
  const auto policy = runtime::LoggingPolicy::create(debug_enabled, level);
  assert(policy.has_value());
  return *policy;
}

void assert_jsonl_schema(std::string_view output, std::size_t expected_records) {
  std::size_t offset = 0U;
  std::size_t records = 0U;
  while (offset < output.size()) {
    const auto end = output.find('\n', offset);
    assert(end != std::string_view::npos);
    assert(aivs::test::is_unified_json_record(output.substr(offset, end - offset)));
    ++records;
    offset = end + 1U;
  }
  assert(records == expected_records);
}

void constrained_json_parser_rejects_non_emitter_grammar() {
  constexpr std::string_view prefix =
      R"({"timestamp":"2026-08-19T00:00:00.000000Z","component":"voice-runtime","level":"info","message":)";
  assert(aivs::test::is_unified_json_record(std::string(prefix) + R"("control\u001b"})"));
  assert(!aivs::test::is_unified_json_record(std::string(prefix) + R"("trailing",})"));
  assert(!aivs::test::is_unified_json_record(std::string(prefix) + R"("slash\/escape"})"));
  assert(!aivs::test::is_unified_json_record(std::string(prefix) + R"("short-control\u000a"})"));
  assert(!aivs::test::is_unified_json_record(std::string(prefix) + R"("lone-high\uD800"})"));
  assert(!aivs::test::is_unified_json_record(std::string(prefix) + R"("lone-low\uDC00"})"));
  assert(!aivs::test::is_unified_json_record(
      std::string(prefix) + R"("surrogate-pair\uD834\uDD1E"})"));
  assert(!aivs::test::is_unified_json_record(std::string(prefix) + "\"raw\nnewline\"}"));
  assert(!aivs::test::is_unified_json_record(
      R"({"timestamp":"2026-08-19T00:00:00.000000Z","component":"voice-runtime","level":"info","message":"overflow","generation":18446744073709551616})"));
}

void schema_escaping_level_gating_flush_and_repeated_instances_are_isolated() {
  const auto directory = unique_directory("Unicode-声音");
  {
    auto initialized = runtime::RuntimeLogger::initialize({
        directory,
        logging_policy(true, runtime::LogLevel::Debug),
        "voice-runtime",
    });
    assert(initialized.logger != nullptr);
    assert(initialized.diagnostic.empty());
    initialized.logger->log(
        runtime::LogLevel::Info,
        "quoted \"line\"\n音乐",
        runtime::LogFields{42U, 7U});
    initialized.logger->log(runtime::LogLevel::Trace, "hidden");
  }

  const auto path = directory / runtime::kRuntimeLogFileName;
  const auto first = read_file(path);
  assert_jsonl_schema(first, 1U);
  assert(first.ends_with('\n'));
  assert(first.find('\n') == first.size() - 1U);
  assert(first.find(R"("timestamp":")") != std::string::npos);
  assert(first.find('T') != std::string::npos);
  assert(first.find(R"(Z","component":"voice-runtime")") != std::string::npos);
  assert(first.find(R"("level":"info")") != std::string::npos);
  assert(first.find(R"("message":"quoted \"line\"\n音乐")") != std::string::npos);
  assert(first.find(R"("request_id":42)") != std::string::npos);
  assert(first.find(R"("generation":7)") != std::string::npos);
  assert(first.find("hidden") == std::string::npos);

  {
    auto second = runtime::RuntimeLogger::initialize({
        directory,
        logging_policy(false, runtime::LogLevel::Error),
        "voice-runtime",
    });
    assert(second.logger != nullptr);
    second.logger->log(runtime::LogLevel::Warn, "hidden warn");
    second.logger->log(runtime::LogLevel::Error, "visible error");
  }
  const auto appended = read_file(path);
  assert_jsonl_schema(appended, 2U);
  assert(appended.find("hidden warn") == std::string::npos);
  assert(appended.find("visible error") != std::string::npos);
  assert(std::count(appended.begin(), appended.end(), '\n') == 2);
  std::filesystem::remove_all(directory);
}

void initialization_failure_is_actionable_and_does_not_leave_registered_state() {
  const auto blocked = unique_directory("blocked");
  {
    std::ofstream output(blocked);
    output << "not a directory";
  }
  auto failure = runtime::RuntimeLogger::initialize({
      blocked,
      logging_policy(false, runtime::LogLevel::Info),
      "voice-runtime",
  });
  assert(failure.logger == nullptr);
  assert(!failure.diagnostic.empty());
  assert(failure.diagnostic.size() <= runtime::kMaximumLogDiagnosticBytes);
  std::filesystem::remove(blocked);

  const std::vector<std::string> invalid_components{
      std::string("voice-\xFF-runtime", 15U),
      std::string("\xC0\xAF", 2U),
      std::string("\xED\xA0\x80", 3U),
      std::string("\xF4\x90\x80\x80", 4U),
      std::string("\xF0\x9F", 2U),
  };
  for (std::size_t index = 0U; index < invalid_components.size(); ++index) {
    const auto invalid_component_directory =
        unique_directory("invalid-component-" + std::to_string(index));
    auto invalid_component = runtime::RuntimeLogger::initialize({
        invalid_component_directory,
        logging_policy(false, runtime::LogLevel::Info),
        invalid_components[index],
    });
    assert(invalid_component.logger == nullptr);
    assert(!invalid_component.diagnostic.empty());
    assert(!std::filesystem::exists(invalid_component_directory));
  }

  const auto recovered = unique_directory("recovered");
  auto success = runtime::RuntimeLogger::initialize({
      recovered,
      logging_policy(false, runtime::LogLevel::Info),
      "voice-runtime",
  });
  assert(success.logger != nullptr);
  success.logger.reset();
  std::filesystem::remove_all(recovered);
}

void component_utf8_byte_boundaries_are_enforced() {
  const auto accepted_directory = unique_directory("component-64");
  const std::string exact_component = std::string(61U, 'a') + "界";
  assert(exact_component.size() == runtime::kMaximumLogComponentBytes);
  {
    auto accepted = runtime::RuntimeLogger::initialize({
        accepted_directory,
        logging_policy(false, runtime::LogLevel::Info),
        exact_component,
    });
    assert(accepted.logger != nullptr);
    accepted.logger->log(runtime::LogLevel::Info, "exact component byte boundary");
  }
  const auto accepted_output =
      read_file(accepted_directory / runtime::kRuntimeLogFileName);
  assert_jsonl_schema(accepted_output, 1U);
  assert(accepted_output.find(exact_component) != std::string::npos);
  std::filesystem::remove_all(accepted_directory);

  const auto rejected_directory = unique_directory("component-65");
  const std::string oversized_component = std::string(62U, 'a') + "界";
  assert(oversized_component.size() == runtime::kMaximumLogComponentBytes + 1U);
  const auto rejected = runtime::RuntimeLogger::initialize({
      rejected_directory,
      logging_policy(false, runtime::LogLevel::Info),
      oversized_component,
  });
  assert(rejected.logger == nullptr);
  assert(!rejected.diagnostic.empty());
  assert(!std::filesystem::exists(rejected_directory));
}

void invalid_message_utf8_is_replaced_before_bounded_jsonl_output() {
  const auto directory = unique_directory("invalid-message");
  {
    auto initialized = runtime::RuntimeLogger::initialize({
        directory,
        logging_policy(false, runtime::LogLevel::Info),
        "voice-runtime",
    });
    assert(initialized.logger != nullptr);
    std::string malformed = "valid-";
    malformed.push_back(static_cast<char>(0xFFU));
    malformed.append("-overlong-");
    malformed.append("\xC0\xAF", 2U);
    malformed.append("-surrogate-");
    malformed.append("\xED\xA0\x80", 3U);
    malformed.append("-out-of-range-");
    malformed.append("\xF4\x90\x80\x80", 4U);
    malformed.append("-continuation-");
    malformed.push_back(static_cast<char>(0x80U));
    malformed.append("-truncated-");
    malformed.push_back(static_cast<char>(0xF0U));
    malformed.push_back(static_cast<char>(0x9FU));
    initialized.logger->log(runtime::LogLevel::Info, malformed);

    std::string exact_boundary(runtime::kMaximumLogMessageBytes - 4U, 'a');
    exact_boundary.append("🎵");
    initialized.logger->log(runtime::LogLevel::Info, exact_boundary);
    std::string truncated_boundary(runtime::kMaximumLogMessageBytes - 3U, 'b');
    truncated_boundary.append("🎵");
    initialized.logger->log(runtime::LogLevel::Info, truncated_boundary);

    const std::string controls{"control\0\x1F", 9U};
    initialized.logger->log(runtime::LogLevel::Info, controls);
  }

  const auto output = read_file(directory / runtime::kRuntimeLogFileName);
  assert_jsonl_schema(output, 4U);
  assert(output.find("valid-\xEF\xBF\xBD-overlong-") != std::string::npos);
  const std::string exact_message =
      "\"message\":\"" + std::string(runtime::kMaximumLogMessageBytes - 4U, 'a') + "🎵\"";
  assert(output.find(exact_message) != std::string::npos);
  const std::string truncated_message =
      "\"message\":\"" + std::string(runtime::kMaximumLogMessageBytes - 3U, 'b') + "\"";
  assert(output.find(truncated_message) != std::string::npos);
  assert(output.find(R"("message":"control\u0000\u001f")") != std::string::npos);
  assert(std::count(output.begin(), output.end(), '\n') == 4);
  std::filesystem::remove_all(directory);
}

}  // namespace

int main() {
  try {
    std::cerr << "Runtime logging phase: policy and parser" << std::endl;
    assert(!runtime::LoggingPolicy::create(false, runtime::LogLevel::Trace).has_value());
    assert(!runtime::LoggingPolicy::create(false, runtime::LogLevel::Debug).has_value());
    assert(!runtime::LoggingPolicy::create(true, static_cast<runtime::LogLevel>(99)).has_value());
    constrained_json_parser_rejects_non_emitter_grammar();
    std::cerr << "Runtime logging phase: schema, flush, and isolation" << std::endl;
    schema_escaping_level_gating_flush_and_repeated_instances_are_isolated();
    std::cerr << "Runtime logging phase: initialization failures" << std::endl;
    initialization_failure_is_actionable_and_does_not_leave_registered_state();
    std::cerr << "Runtime logging phase: component UTF-8 boundaries" << std::endl;
    component_utf8_byte_boundaries_are_enforced();
    std::cerr << "Runtime logging phase: invalid message UTF-8" << std::endl;
    invalid_message_utf8_is_replaced_before_bounded_jsonl_output();
    return 0;
  } catch (const std::exception& error) {
    std::cerr << error.what() << std::endl;
    return 1;
  }
}
