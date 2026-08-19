#ifdef NDEBUG
#undef NDEBUG
#endif

#include <algorithm>
#include <cassert>
#include <cctype>
#include <chrono>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <ai_voice_runtime/logging.hpp>

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

std::size_t valid_utf8_sequence_length(std::string_view value, std::size_t offset) {
  const auto available = value.size() - offset;
  const auto first = static_cast<unsigned char>(value[offset]);
  const auto continuation = [&](std::size_t index) {
    return index < available &&
           (static_cast<unsigned char>(value[offset + index]) & 0xC0U) == 0x80U;
  };
  if (first <= 0x7FU) {
    return 1U;
  }
  if (first >= 0xC2U && first <= 0xDFU && continuation(1U)) {
    return 2U;
  }
  if (available >= 3U && continuation(1U) && continuation(2U) &&
      ((first == 0xE0U && static_cast<unsigned char>(value[offset + 1U]) >= 0xA0U) ||
       (first >= 0xE1U && first <= 0xECU) ||
       (first == 0xEDU && static_cast<unsigned char>(value[offset + 1U]) <= 0x9FU) ||
       (first >= 0xEEU && first <= 0xEFU))) {
    return 3U;
  }
  if (available >= 4U && continuation(1U) && continuation(2U) && continuation(3U) &&
      ((first == 0xF0U && static_cast<unsigned char>(value[offset + 1U]) >= 0x90U) ||
       (first >= 0xF1U && first <= 0xF3U) ||
       (first == 0xF4U && static_cast<unsigned char>(value[offset + 1U]) <= 0x8FU))) {
    return 4U;
  }
  return 0U;
}

bool is_valid_utf8(std::string_view value) {
  for (std::size_t offset = 0U; offset < value.size();) {
    const auto length = valid_utf8_sequence_length(value, offset);
    if (length == 0U) {
      return false;
    }
    offset += length;
  }
  return true;
}

enum class JsonValueKind {
  String,
  Unsigned,
};

struct JsonField {
  std::string name;
  JsonValueKind kind;
};

bool parse_json_string(std::string_view input, std::size_t& offset, std::string* decoded) {
  if (offset == input.size() || input[offset++] != '"') {
    return false;
  }
  while (offset < input.size()) {
    const auto character = static_cast<unsigned char>(input[offset++]);
    if (character == '"') {
      return true;
    }
    if (character < 0x20U) {
      return false;
    }
    if (character != '\\') {
      if (decoded != nullptr) {
        decoded->push_back(static_cast<char>(character));
      }
      continue;
    }
    if (offset == input.size()) {
      return false;
    }
    const auto escaped = input[offset++];
    if (escaped == 'u') {
      for (std::size_t digit = 0U; digit < 4U; ++digit) {
        if (offset == input.size() || !std::isxdigit(static_cast<unsigned char>(input[offset++]))) {
          return false;
        }
      }
      if (decoded != nullptr) {
        decoded->push_back('?');
      }
      continue;
    }
    if (std::string_view{R"("\/bfnrt)"}.find(escaped) == std::string_view::npos) {
      return false;
    }
    if (decoded != nullptr) {
      decoded->push_back(escaped);
    }
  }
  return false;
}

std::vector<JsonField> parse_json_object(std::string_view input) {
  std::vector<JsonField> fields;
  std::size_t offset = 0U;
  if (input.empty() || input[offset++] != '{') {
    return {};
  }
  while (offset < input.size() && input[offset] != '}') {
    std::string name;
    if (!parse_json_string(input, offset, &name) || offset == input.size() ||
        input[offset++] != ':') {
      return {};
    }
    JsonValueKind kind{};
    if (offset < input.size() && input[offset] == '"') {
      kind = JsonValueKind::String;
      if (!parse_json_string(input, offset, nullptr)) {
        return {};
      }
    } else {
      kind = JsonValueKind::Unsigned;
      const auto begin = offset;
      while (offset < input.size() && input[offset] >= '0' && input[offset] <= '9') {
        ++offset;
      }
      if (begin == offset || (input[begin] == '0' && offset - begin != 1U)) {
        return {};
      }
    }
    if (std::any_of(fields.begin(), fields.end(), [&](const JsonField& field) {
          return field.name == name;
        })) {
      return {};
    }
    fields.push_back({std::move(name), kind});
    if (offset < input.size() && input[offset] == ',') {
      ++offset;
    } else {
      break;
    }
  }
  if (offset == input.size() || input[offset++] != '}' || offset != input.size()) {
    return {};
  }
  return fields;
}

void assert_jsonl_schema(std::string_view output, std::size_t expected_records) {
  std::size_t offset = 0U;
  std::size_t records = 0U;
  while (offset < output.size()) {
    const auto end = output.find('\n', offset);
    assert(end != std::string_view::npos);
    const auto line = output.substr(offset, end - offset);
    assert(is_valid_utf8(line));
    const auto fields = parse_json_object(line);
    assert(!fields.empty());
    const auto has = [&](std::string_view name, JsonValueKind kind) {
      return std::count_if(fields.begin(), fields.end(), [&](const JsonField& field) {
               return field.name == name && field.kind == kind;
             }) == 1;
    };
    assert(has("timestamp", JsonValueKind::String));
    assert(has("component", JsonValueKind::String));
    assert(has("level", JsonValueKind::String));
    assert(has("message", JsonValueKind::String));
    for (const auto& field : fields) {
      assert(field.name == "timestamp" || field.name == "component" ||
             field.name == "level" || field.name == "message" ||
             ((field.name == "request_id" || field.name == "generation") &&
              field.kind == JsonValueKind::Unsigned));
    }
    ++records;
    offset = end + 1U;
  }
  assert(records == expected_records);
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
  assert(!runtime::LoggingPolicy::create(false, runtime::LogLevel::Trace).has_value());
  assert(!runtime::LoggingPolicy::create(false, runtime::LogLevel::Debug).has_value());
  assert(!runtime::LoggingPolicy::create(true, static_cast<runtime::LogLevel>(99)).has_value());
  schema_escaping_level_gating_flush_and_repeated_instances_are_isolated();
  initialization_failure_is_actionable_and_does_not_leave_registered_state();
  invalid_message_utf8_is_replaced_before_bounded_jsonl_output();
}
