/**
 * @file unity.h
 * @brief Single-header C unit test framework.
 *
 * Usage:
 *   UNITY_BEGIN("Suite");
 *   RUN_TEST(test_my_function);
 *   return UNITY_END();
 */

#ifndef UNITY_H
#define UNITY_H

#include <stdio.h>
#include <string.h>
#include <stdint.h>

#define UNITY_GREEN "\033[0;32m"
#define UNITY_RED "\033[0;31m"
#define UNITY_YELLOW "\033[0;33m"
#define UNITY_RESET "\033[0m"

static int unity_run = 0;
static int unity_passed = 0;
static int unity_failed = 0;

#define UNITY_BEGIN(name)                                                \
    do                                                                   \
    {                                                                    \
        unity_run = unity_passed = unity_failed = 0;                     \
        printf("\n" UNITY_YELLOW "=== %s ===" UNITY_RESET "\n\n", name); \
    } while (0)

#define UNITY_END()                                              \
    (printf("\n--- %d run  " UNITY_GREEN "%d passed" UNITY_RESET \
            "  " UNITY_RED "%d failed" UNITY_RESET "\n\n",       \
            unity_run, unity_passed, unity_failed),              \
     (unity_failed > 0 ? 1 : 0))

#define RUN_TEST(fn)                                     \
    do                                                   \
    {                                                    \
        unity_run++;                                     \
        int _f = unity_failed;                           \
        printf("  %-45s ", #fn);                         \
        fn();                                            \
        if (unity_failed == _f)                          \
        {                                                \
            unity_passed++;                              \
            printf(UNITY_GREEN "PASS" UNITY_RESET "\n"); \
        }                                                \
    } while (0)

#define _FAIL(msg)                                                    \
    do                                                                \
    {                                                                 \
        unity_failed++;                                               \
        printf(UNITY_RED "FAIL\n    >> %s" UNITY_RESET "  (%s:%d)\n", \
               msg, __FILE__, __LINE__);                              \
        return;                                                       \
    } while (0)

#define TEST_ASSERT(c)                      \
    do                                      \
    {                                       \
        if (!(c))                           \
            _FAIL("Assertion failed: " #c); \
    } while (0)
#define TEST_ASSERT_NOT_NULL(p)              \
    do                                       \
    {                                        \
        if ((p) == NULL)                     \
            _FAIL("Expected non-NULL: " #p); \
    } while (0)
#define TEST_ASSERT_NULL(p)              \
    do                                   \
    {                                    \
        if ((p) != NULL)                 \
            _FAIL("Expected NULL: " #p); \
    } while (0)

#define TEST_ASSERT_EQUAL_INT(e, a)                                 \
    do                                                              \
    {                                                               \
        int _e = (int)(e), _a = (int)(a);                           \
        if (_e != _a)                                               \
        {                                                           \
            char _m[128];                                           \
            snprintf(_m, sizeof(_m), "Expected %d got %d", _e, _a); \
            _FAIL(_m);                                              \
        }                                                           \
    } while (0)

#define TEST_ASSERT_EQUAL_UINT8(e, a)                                       \
    do                                                                      \
    {                                                                       \
        uint8_t _e = (uint8_t)(e), _a = (uint8_t)(a);                       \
        if (_e != _a)                                                       \
        {                                                                   \
            char _m[128];                                                   \
            snprintf(_m, sizeof(_m), "Expected 0x%02X got 0x%02X", _e, _a); \
            _FAIL(_m);                                                      \
        }                                                                   \
    } while (0)

#define TEST_ASSERT_EQUAL_UINT32(e, a)                                      \
    do                                                                      \
    {                                                                       \
        uint32_t _e = (uint32_t)(e), _a = (uint32_t)(a);                    \
        if (_e != _a)                                                       \
        {                                                                   \
            char _m[128];                                                   \
            snprintf(_m, sizeof(_m), "Expected 0x%08X got 0x%08X", _e, _a); \
            _FAIL(_m);                                                      \
        }                                                                   \
    } while (0)

#define TEST_ASSERT_EQUAL_STRING(e, a)                             \
    do                                                             \
    {                                                              \
        const char *_e = (e), *_a = (a);                           \
        if (!_e || !_a || strcmp(_e, _a) != 0)                     \
        {                                                          \
            char _m[256];                                          \
            snprintf(_m, sizeof(_m), "Expected \"%s\" got \"%s\"", \
                     _e ? _e : "(null)", _a ? _a : "(null)");      \
            _FAIL(_m);                                             \
        }                                                          \
    } while (0)

#endif /* UNITY_H */
