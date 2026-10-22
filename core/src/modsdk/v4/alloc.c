/*
 * A small allocator and the memory functions the compiler may call. The
 * module has no libc, so the template brings them.
 *
 * mod_alloc takes memory from a fixed block in the module's data. eidos
 * keeps its own heap in the module's memory above the data, so the module
 * must not grow its memory or take memory past its data. Change
 * MOD_HEAP_BYTES for a bigger block. mod_free only reuses the last block;
 * a mod that frees often should keep its own pools.
 */

#include "eidos_wasm.h"

#define MOD_HEAP_BYTES (512u * 1024u)

static unsigned char heap[MOD_HEAP_BYTES] __attribute__((aligned(8)));
static size_t heap_top;
static size_t last_block = (size_t)-1;

void *mod_alloc(size_t size)
{
    size_t at = (heap_top + 7u) & ~(size_t)7u;

    if (at > MOD_HEAP_BYTES || size > MOD_HEAP_BYTES - at) {
        return NULL;
    }
    heap_top = at + size;
    last_block = at;
    return &heap[at];
}

void mod_free(void *block)
{
    if (block != NULL && last_block != (size_t)-1 &&
        (unsigned char *)block == &heap[last_block]) {
        heap_top = last_block;
        last_block = (size_t)-1;
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
