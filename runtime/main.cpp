#include <ai_voice_runtime/stdio_runtime.hpp>

int main(int argc, char** argv) {
  return static_cast<int>(ai_voice::runtime::run_native_stdio(argc, argv));
}
