#ifdef NDEBUG
#undef NDEBUG
#endif

#include <algorithm>
#include <cassert>
#include <chrono>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>

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

void schema_escaping_level_gating_flush_and_repeated_instances_are_isolated() {
  const auto directory = unique_directory("Unicode-声音");
  {
    auto initialized = runtime::RuntimeLogger::initialize({
        directory,
        runtime::LogLevel::Debug,
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
        runtime::LogLevel::Error,
        "voice-runtime",
    });
    assert(second.logger != nullptr);
    second.logger->log(runtime::LogLevel::Warn, "hidden warn");
    second.logger->log(runtime::LogLevel::Error, "visible error");
  }
  const auto appended = read_file(path);
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
      runtime::LogLevel::Info,
      "voice-runtime",
  });
  assert(failure.logger == nullptr);
  assert(!failure.diagnostic.empty());
  assert(failure.diagnostic.size() <= runtime::kMaximumLogDiagnosticBytes);
  std::filesystem::remove(blocked);

  const auto recovered = unique_directory("recovered");
  auto success = runtime::RuntimeLogger::initialize({
      recovered,
      runtime::LogLevel::Info,
      "voice-runtime",
  });
  assert(success.logger != nullptr);
  success.logger.reset();
  std::filesystem::remove_all(recovered);
}

}  // namespace

int main() {
  schema_escaping_level_gating_flush_and_repeated_instances_are_isolated();
  initialization_failure_is_actionable_and_does_not_leave_registered_state();
}
