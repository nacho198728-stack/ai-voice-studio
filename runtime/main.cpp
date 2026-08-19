#include <ai_voice_runtime/stdio_runtime.hpp>

#if defined(_WIN32)
int wmain(int argc, wchar_t** argv) {
#else
int main(int argc, char** argv) {
#endif
  return static_cast<int>(ai_voice::runtime::run_native_stdio(argc, argv));
}
