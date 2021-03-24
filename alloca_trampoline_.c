#include <stddef.h>

void _alloca_trampoline(size_t num, void (*callback)(void* restrict ptr, void* data), void* data)
{
    unsigned char ptr[num];
    callback(ptr, data);
} // C99 mandates empty line at EOF

