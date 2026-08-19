#pragma once

#include <algorithm>
#include <charconv>
#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>
#include <system_error>
#include <utility>
#include <vector>

namespace aivs::test {

enum class JsonValueKind {
  String,
  Unsigned,
};

struct JsonField {
  std::string name;
  JsonValueKind kind;
  std::string string_value;
};

inline std::size_t valid_utf8_sequence_length(std::string_view value, std::size_t offset) {
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

inline bool is_valid_utf8(std::string_view value) {
  for (std::size_t offset = 0U; offset < value.size();) {
    const auto length = valid_utf8_sequence_length(value, offset);
    if (length == 0U) {
      return false;
    }
    offset += length;
  }
  return true;
}

inline int hex_value(char value) {
  if (value >= '0' && value <= '9') {
    return value - '0';
  }
  if (value >= 'a' && value <= 'f') {
    return value - 'a' + 10;
  }
  return -1;
}

inline bool parse_emitted_json_string(
    std::string_view input,
    std::size_t& offset,
    std::string& decoded,
    bool allow_escapes) {
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
      decoded.push_back(static_cast<char>(character));
      continue;
    }
    if (!allow_escapes || offset == input.size()) {
      return false;
    }
    const auto escaped = input[offset++];
    switch (escaped) {
      case '"':
        decoded.push_back('"');
        break;
      case '\\':
        decoded.push_back('\\');
        break;
      case 'b':
        decoded.push_back('\b');
        break;
      case 'f':
        decoded.push_back('\f');
        break;
      case 'n':
        decoded.push_back('\n');
        break;
      case 'r':
        decoded.push_back('\r');
        break;
      case 't':
        decoded.push_back('\t');
        break;
      case 'u': {
        if (offset + 4U > input.size() || input[offset] != '0' || input[offset + 1U] != '0') {
          return false;
        }
        const auto high = hex_value(input[offset + 2U]);
        const auto low = hex_value(input[offset + 3U]);
        if (high < 0 || low < 0) {
          return false;
        }
        const auto code = static_cast<unsigned int>(high * 16 + low);
        if (code > 0x1FU || code == 0x08U || code == 0x09U || code == 0x0AU ||
            code == 0x0CU || code == 0x0DU) {
          return false;
        }
        decoded.push_back(static_cast<char>(code));
        offset += 4U;
        break;
      }
      default:
        return false;
    }
  }
  return false;
}

inline std::vector<JsonField> parse_emitted_json_object(std::string_view input) {
  std::vector<JsonField> fields;
  std::size_t offset = 0U;
  if (input.empty() || input[offset++] != '{') {
    return {};
  }
  while (offset < input.size() && input[offset] != '}') {
    std::string name;
    if (!parse_emitted_json_string(input, offset, name, false) || offset == input.size() ||
        input[offset++] != ':') {
      return {};
    }
    JsonField field{std::move(name), JsonValueKind::Unsigned, {}};
    if (offset < input.size() && input[offset] == '"') {
      field.kind = JsonValueKind::String;
      if (!parse_emitted_json_string(input, offset, field.string_value, true)) {
        return {};
      }
    } else {
      const auto begin = offset;
      while (offset < input.size() && input[offset] >= '0' && input[offset] <= '9') {
        ++offset;
      }
      if (begin == offset || (input[begin] == '0' && offset - begin != 1U)) {
        return {};
      }
      std::uint64_t value = 0U;
      const auto [end, error] =
          std::from_chars(input.data() + begin, input.data() + offset, value);
      if (error != std::errc{} || end != input.data() + offset) {
        return {};
      }
    }
    if (std::any_of(fields.begin(), fields.end(), [&](const JsonField& existing) {
          return existing.name == field.name;
        })) {
      return {};
    }
    fields.push_back(std::move(field));
    if (offset < input.size() && input[offset] == ',') {
      ++offset;
      if (offset == input.size() || input[offset] == '}') {
        return {};
      }
    } else {
      break;
    }
  }
  if (offset == input.size() || input[offset++] != '}' || offset != input.size()) {
    return {};
  }
  return fields;
}

inline const JsonField* find_field(
    const std::vector<JsonField>& fields,
    std::string_view name,
    JsonValueKind kind) {
  const auto found = std::find_if(fields.begin(), fields.end(), [&](const JsonField& field) {
    return field.name == name && field.kind == kind;
  });
  return found == fields.end() ? nullptr : &*found;
}

inline bool is_unified_json_record(std::string_view input) {
  if (!is_valid_utf8(input)) {
    return false;
  }
  const auto fields = parse_emitted_json_object(input);
  if (fields.empty()) {
    return false;
  }
  const auto* timestamp = find_field(fields, "timestamp", JsonValueKind::String);
  const auto* component = find_field(fields, "component", JsonValueKind::String);
  const auto* level = find_field(fields, "level", JsonValueKind::String);
  const auto* message = find_field(fields, "message", JsonValueKind::String);
  if (timestamp == nullptr || component == nullptr || level == nullptr || message == nullptr ||
      timestamp->string_value.find('T') == std::string::npos ||
      timestamp->string_value.find('.') == std::string::npos ||
      !timestamp->string_value.ends_with('Z') || component->string_value.empty() ||
      component->string_value.size() > 64U || message->string_value.size() > 512U ||
      (level->string_value != "trace" && level->string_value != "debug" &&
       level->string_value != "info" && level->string_value != "warn" &&
       level->string_value != "error")) {
    return false;
  }
  return std::all_of(fields.begin(), fields.end(), [](const JsonField& field) {
    return field.name == "timestamp" || field.name == "component" || field.name == "level" ||
           field.name == "message" ||
           ((field.name == "request_id" || field.name == "generation") &&
            field.kind == JsonValueKind::Unsigned);
  });
}

}  // namespace aivs::test
