/*
 * A code mod. It is built to mod.wasm and loaded by eidos.
 *
 * This example logs when it starts and when a battle begins. It changes
 * nothing in the game. Add handlers with eidos_on, one per event. A handler
 * is registered by its function table index, which is its address.
 */

#include "eidos_wasm.h"

static void say(const char *text)
{
    uint32_t length = 0;
    while (text[length] != '\0') {
        length++;
    }
    eidos_log(text, length);
}

/* Runs when a battle starts. */
static int32_t on_battle_start(EidosEvent *event)
{
    (void)event;
    say("a battle started");
    return EIDOS_EVENT_CONTINUE;
}

EIDOS_EXPORT("eidos_mod_start")
int32_t eidos_mod_start(void)
{
    say("mod started");
    return eidos_on(EIDOS_EVENT_BATTLE_START, (uint32_t)(uintptr_t)on_battle_start);
}

EIDOS_EXPORT("eidos_mod_stop")
void eidos_mod_stop(void)
{
    say("mod stopped");
}
