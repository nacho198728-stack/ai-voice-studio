#if defined(_WIN32)
#define AIVS_TEST_EXPORT extern "C" __declspec(dllexport)
#elif defined(__GNUC__) || defined(__clang__)
#define AIVS_TEST_EXPORT extern "C" __attribute__((visibility("default")))
#else
#define AIVS_TEST_EXPORT extern "C"
#endif

AIVS_TEST_EXPORT int aivs_test_not_a_factory() {
  return 1;
}
