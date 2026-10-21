/*
 * A small allocator and the memory functions the compiler may call. The
 * module has no libc, so the template brings them.
 *
 * mod_alloc takes memory from the end of the module's data and grows the
 * module's memory when it needs more. mod_free only reuses the last block;
 * a mod that frees often should keep its own pools.
 */

#include <stddef.h>
#include "eidos_wasm.h"

extern unsigned char __heap_base;

static uintptr_t heap_top;
static uintptr_t last_block;

void *mod_alloc(size_t size)
{
    uintptr_t at;
    uintptr_t end;
    size_t have;

    if (heap_top == 0) {
        heap_top = (uintptr_t)&__heap_base;
    }
    at = (heap_top + 7u) & ~(uintptr_t)7u;
    end = at + size;
    if (end < at) {
        return NULL;
    }
    have = (size_t)__builtin_wasm_memory_size(0) * 65536u;
    if (end > have) {
        size_t pages = (end - have + 65535u) / 65536u;
        if (__builtin_wasm_memory_grow(0, pages) == (size_t)-1) {
            return NULL;
        }
    }
    heap_top = end;
    last_block = at;
    return (void *)at;
}

void mod_free(void *block)
{
    if ((uintptr_t)block == last_block && block != NULL) {
        heap_top = last_block;
        last_block = 0;
    }
}

void *memcpy(void *dst, const void *src, size_t count)
{
    unsigned char *d = dst;
    const unsigned char *s = src;
    while (count--) {
        *d++ = *s++;
    }
    return dst;
}

void *memmove(void *dst, const void *src, size_t count)
{
    unsigned char *d = dst;
    const unsigned char *s = src;
    if (d < s) {
        while (count--) {
            *d++ = *s++;
        }
    } else {
        d += count;
        s += count;
        while (count--) {
            *--d = *--s;
        }
    }
    return dst;
}

void *memset(void *dst, int value, size_t count)
{
    unsigned char *d = dst;
    while (count--) {
        *d++ = (unsigned char)value;
    }
    return dst;
}

int memcmp(const void *a, const void *b, size_t count)
{
    const unsigned char *x = a;
    const unsigned char *y = b;
    while (count--) {
        if (*x != *y) {
            return *x - *y;
        }
        x++;
        y++;
    }
    return 0;
}
