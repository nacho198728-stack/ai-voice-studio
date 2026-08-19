#ifdef NDEBUG
#undef NDEBUG
#endif

#include <cassert>

#if defined(_WIN32)

int main() {
  // Windows has no POSIX FIFO. The instance-owned error-handler code still
  // compiles here; Windows filesystem fault injection remains a CI extension.
  return 77;
}

#else

#include <array>
#include <chrono>
#include <csignal>
#include <cstdint>
#include <filesystem>
#include <string>
#include <string_view>
#include <vector>

#include <fcntl.h>
#include <poll.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include <ai_voice_contracts/runtime_message.hpp>

#include "jsonl_test_support.hpp"

namespace message = ai_voice::contracts::runtime_message;

namespace {

struct ChildPipes {
  pid_t pid;
  int stdin_write;
  int stdout_read;
  int stderr_read;
};

void close_fd(int descriptor) {
  if (descriptor >= 0) {
    assert(::close(descriptor) == 0);
  }
}

ChildPipes spawn_runtime(const char* runtime, const std::filesystem::path& directory) {
  std::array<int, 2> stdin_pipe{};
  std::array<int, 2> stdout_pipe{};
  std::array<int, 2> stderr_pipe{};
  assert(::pipe(stdin_pipe.data()) == 0);
  assert(::pipe(stdout_pipe.data()) == 0);
  assert(::pipe(stderr_pipe.data()) == 0);
  const auto pid = ::fork();
  assert(pid >= 0);
  if (pid == 0) {
    assert(::signal(SIGPIPE, SIG_IGN) != SIG_ERR);
    assert(::dup2(stdin_pipe[0], STDIN_FILENO) >= 0);
    assert(::dup2(stdout_pipe[1], STDOUT_FILENO) >= 0);
    assert(::dup2(stderr_pipe[1], STDERR_FILENO) >= 0);
    for (const auto descriptor : {
             stdin_pipe[0], stdin_pipe[1], stdout_pipe[0], stdout_pipe[1], stderr_pipe[0],
             stderr_pipe[1]}) {
      ::close(descriptor);
    }
    const auto directory_text = directory.string();
    ::execl(
        runtime,
        runtime,
        "--log-directory",
        directory_text.c_str(),
        "--log-level",
        "info",
        "--debug-enabled",
        "false",
        "--generation",
        "77",
        static_cast<char*>(nullptr));
    ::_exit(127);
  }
  close_fd(stdin_pipe[0]);
  close_fd(stdout_pipe[1]);
  close_fd(stderr_pipe[1]);
  return {pid, stdin_pipe[1], stdout_pipe[0], stderr_pipe[0]};
}

void read_available(int descriptor, std::vector<std::uint8_t>& output, bool& open) {
  std::array<std::uint8_t, 4096> buffer{};
  const auto count = ::read(descriptor, buffer.data(), buffer.size());
  if (count > 0) {
    output.insert(output.end(), buffer.begin(), buffer.begin() + count);
  } else if (count == 0) {
    close_fd(descriptor);
    open = false;
  }
}

void drain_pipes(
    int stdout_read,
    int stderr_read,
    std::vector<std::uint8_t>& stdout_bytes,
    std::vector<std::uint8_t>& stderr_bytes) {
  bool stdout_open = true;
  bool stderr_open = true;
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
  while (stdout_open || stderr_open) {
    assert(std::chrono::steady_clock::now() < deadline);
    std::array<pollfd, 2> descriptors{{
        {stdout_open ? stdout_read : -1, static_cast<short>(POLLIN | POLLHUP), 0},
        {stderr_open ? stderr_read : -1, static_cast<short>(POLLIN | POLLHUP), 0},
    }};
    assert(::poll(descriptors.data(), descriptors.size(), 100) >= 0);
    if (stdout_open && (descriptors[0].revents & (POLLIN | POLLHUP)) != 0) {
      read_available(stdout_read, stdout_bytes, stdout_open);
    }
    if (stderr_open && (descriptors[1].revents & (POLLIN | POLLHUP)) != 0) {
      read_available(stderr_read, stderr_bytes, stderr_open);
    }
  }
}

void assert_stderr_jsonl(std::string_view stderr_text, std::string_view forbidden_path) {
  assert(stderr_text.find("LOG ERROR") == std::string_view::npos);
  assert(stderr_text.find(forbidden_path) == std::string_view::npos);
  std::size_t offset = 0U;
  std::size_t records = 0U;
  while (offset < stderr_text.size()) {
    const auto end = stderr_text.find('\n', offset);
    assert(end != std::string_view::npos);
    assert(aivs::test::is_unified_json_record(stderr_text.substr(offset, end - offset)));
    ++records;
    offset = end + 1U;
  }
  assert(records >= 2U);
}

}  // namespace

int main(int argc, char** argv) {
  assert(argc == 2);
  const auto nonce = std::chrono::steady_clock::now().time_since_epoch().count();
  const auto directory = std::filesystem::temp_directory_path() /
                         ("aivs-late-log-failure-" + std::to_string(nonce));
  assert(std::filesystem::create_directories(directory));
  const auto fifo = directory / "voice-runtime.jsonl";
  assert(::mkfifo(fifo.c_str(), 0600) == 0);
  const auto fifo_reader = ::open(fifo.c_str(), O_RDONLY | O_NONBLOCK);
  assert(fifo_reader >= 0);
  const auto descriptor_flags = ::fcntl(fifo_reader, F_GETFD);
  assert(descriptor_flags >= 0);
  assert(::fcntl(fifo_reader, F_SETFD, descriptor_flags | FD_CLOEXEC) == 0);

  auto child = spawn_runtime(argv[1], directory);
  std::vector<std::uint8_t> stdout_bytes;
  pollfd hello_poll{child.stdout_read, POLLIN, 0};
  assert(::poll(&hello_poll, 1U, 5000) == 1);
  assert((hello_poll.revents & POLLIN) != 0);
  bool stdout_open = true;
  read_available(child.stdout_read, stdout_bytes, stdout_open);
  assert(stdout_open);

  close_fd(fifo_reader);
  close_fd(child.stdin_write);
  std::vector<std::uint8_t> stderr_bytes;
  drain_pipes(child.stdout_read, child.stderr_read, stdout_bytes, stderr_bytes);

  int status = 0;
  assert(::waitpid(child.pid, &status, 0) == child.pid);
  assert(WIFEXITED(status));
  assert(WEXITSTATUS(status) == 0);

  message::Decoder decoder;
  const auto decoded = decoder.feed(stdout_bytes);
  assert(!decoded.error.has_value());
  assert(!decoder.finish().has_value());
  assert(decoded.messages.size() == 1U);
  assert(decoded.messages[0].kind == message::MessageKind::Hello);
  assert(decoded.messages[0].request_id == 0U);

  const std::string stderr_text(stderr_bytes.begin(), stderr_bytes.end());
  assert_stderr_jsonl(stderr_text, directory.string());
  assert(std::filesystem::remove(fifo));
  assert(std::filesystem::remove(directory));
}

#endif
