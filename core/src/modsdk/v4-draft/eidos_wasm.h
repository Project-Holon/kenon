/*
 * eidos_wasm.h: what a code mod can call and must export.
 *
 * DRAFT. This file follows eidos's design note (docs/mod-code-wasm.md) and
 * stands in until eidos publishes sdk/api/v4. Its function names, argument
 * lists and event numbers can change then. The event numbers are the ones of
 * eidos's sdk/mod_api.h (BW_EVENT_*).
 *
 * A mod is built for wasm32-freestanding: no libc, no WASI. It reaches the
 * game only through the imports below.
 */

#ifndef EIDOS_WASM_H
#define EIDOS_WASM_H

#include <stddef.h>
#include <stdint.h>

#define EIDOS_API_VERSION 4

#define EIDOS_IMPORT(name) \
    __attribute__((import_module("eidos"), import_name(name)))
#define EIDOS_EXPORT(name) __attribute__((export_name(name)))

/* The start of every event payload. */
typedef struct EidosEvent {
    uint32_t size; /* size of the whole payload */
    uint32_t kind; /* the EIDOS_EVENT_* number */
} EidosEvent;

/* What a handler returns. */
enum {
    EIDOS_CONTINUE = 0, /* the game goes on */
    EIDOS_REFUSE = 1    /* the game takes the refused outcome */
};

/* A handler. The pointer is a function table index in the module. */
typedef int32_t (*EidosHandler)(const EidosEvent *event);

/* The events. */
enum {
    /* session */
    EIDOS_EVENT_GAME_NEW = 1,
    EIDOS_EVENT_SAVE_BEGIN = 2,
    EIDOS_EVENT_SAVE_DONE = 3,
    EIDOS_EVENT_SAVE_LOAD = 4,
    EIDOS_EVENT_GAME_CLEARED = 5,

    /* world */
    EIDOS_EVENT_ROUTE_CHANGE = 10,
    EIDOS_EVENT_MAP_WARP = 11,
    EIDOS_EVENT_PLAYER_STEP = 12,
    EIDOS_EVENT_TRAINER_SPOTTED = 13,
    EIDOS_EVENT_NPC_TALK = 14,
    EIDOS_EVENT_MENU_OPEN = 15,
    EIDOS_EVENT_MENU_BUILD = 16,
    EIDOS_EVENT_FIELD_MOVE = 17,
    EIDOS_EVENT_PC_OPEN = 18,

    /* encounters */
    EIDOS_EVENT_ENCOUNTER_WILD = 30,
    EIDOS_EVENT_ENCOUNTER_STATIC = 31,
    EIDOS_EVENT_ENCOUNTER_TRAINER = 32,

    /* pokemon */
    EIDOS_EVENT_MON_CREATE = 40,
    EIDOS_EVENT_MON_RECEIVE = 41,
    EIDOS_EVENT_MON_NICKNAME = 42,
    EIDOS_EVENT_MON_EVOLVE = 43,
    EIDOS_EVENT_MON_EXP = 44,
    EIDOS_EVENT_MON_LEVELUP = 45,
    EIDOS_EVENT_MOVE_LEARN = 46,
    EIDOS_EVENT_MON_RELEASE = 47,
    EIDOS_EVENT_BOX_DEPOSIT = 48,
    EIDOS_EVENT_BOX_WITHDRAW = 49,
    EIDOS_EVENT_PARTY_SWAP = 50,
    EIDOS_EVENT_MON_HELDITEM = 51,
    EIDOS_EVENT_EGG_HATCH = 52,
    EIDOS_EVENT_TRADE_INGAME = 53,

    /* battle */
    EIDOS_EVENT_BATTLE_START = 70,
    EIDOS_EVENT_BATTLE_ACTION = 71,
    EIDOS_EVENT_BATTLE_ITEM = 72,
    EIDOS_EVENT_BATTLE_SWITCH = 73,
    EIDOS_EVENT_BATTLE_ESCAPE = 74,
    EIDOS_EVENT_BATTLE_MOVE = 75,
    EIDOS_EVENT_BATTLE_DAMAGE = 76,
    EIDOS_EVENT_BATTLE_FAINT = 77,
    EIDOS_EVENT_CATCH_ATTEMPT = 78,
    EIDOS_EVENT_CATCH_RESULT = 79,
    EIDOS_EVENT_BATTLE_END = 80,
    EIDOS_EVENT_WHITEOUT = 81,

    /* items, money and healing */
    EIDOS_EVENT_ITEM_ADD = 90,
    EIDOS_EVENT_ITEM_USE = 91,
    EIDOS_EVENT_SHOP_BUY = 92,
    EIDOS_EVENT_MONEY_CHANGE = 93,
    EIDOS_EVENT_HEAL = 94,

    /* progress */
    EIDOS_EVENT_BADGE_GET = 110,
    EIDOS_EVENT_FLAG_SET = 111,
    EIDOS_EVENT_DEX_REGISTER = 112
};

/* Imports: functions the game provides (import module "eidos"). */

/* Writes a line to the mod log. */
EIDOS_IMPORT("log") void eidos_log(const char *text, uint32_t length);

/* Runs `handler` when the event happens. Returns 0 when it is set. */
EIDOS_IMPORT("on") int32_t eidos_on(uint32_t kind, EidosHandler handler);

/* Replaces a declared game function with `handler`. `key` names the
   function. Returns 0 when it is set. */
EIDOS_IMPORT("override") int32_t eidos_override(uint32_t key, void *handler);

/* Exports: functions the mod must define. */

/* Called once when the mod loads. Return 0 when it is ready. */
int32_t eidos_mod_start(void);

/* Called once when the mod unloads. */
void eidos_mod_stop(void);

/* A small allocator (alloc.c in the template). */
void *mod_alloc(size_t size);
void mod_free(void *block);

#endif
