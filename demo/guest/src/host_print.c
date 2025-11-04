#include "host_print.h"

#include <printf.h>

int host_print(const char *s, size_t len) {
    return printf("%.*s", (int)len, s);
}
