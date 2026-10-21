// kenon's own text for the mod docs: when each event fires, what each
// function does, the rules-language pages and the recipes. tools/mod-docs.mjs
// merges it with what it reads from eidos and writes api.json. Facts about
// the API come from eidos (docs/mod-*.md, sdk/*.h); only add text here that
// you can trace to one of those.

const rule = (s) => s.replace(/^\n/, "").replace(/\n\s*$/, "\n");

// Entry points shared by the C recipes (eidos_wasm.h, api 4). A handler is
// registered by its function table index, which is its address.
const start = (reg) => `EIDOS_EXPORT("eidos_mod_start")
int32_t eidos_mod_start(void)
{
${reg}
}

EIDOS_EXPORT("eidos_mod_stop")
void eidos_mod_stop(void)
{
}
`;

// The place of a Pokemon reference goes to the state calls as five arguments.
const mon = (m, arrow = ".") => `${m}${arrow}handle, ${m}${arrow}place, ${m}${arrow}side, ${m}${arrow}box, ${m}${arrow}slot`;

export default {
  // The titles of the constant groups of sdk/api/v4/enums.json, by the prefix
  // of the BW_ name. They are the comments of eidos's mod_api.h.
  enumTitles: {
    BW_MON_: "Where a Pokemon is.",
    BW_EVENT_: "What a handler returns.",
    BW_ENCOUNTER_: "How an encounter was rolled (encounter.wild).",
    BW_RECEIVE_: "How a Pokemon reached the player (mon.receive).",
    BW_FORMAT_: "The battle format (battle.start, encounter.trainer).",
    BW_ACTION_: "What the player chose in battle (battle.action).",
    BW_OUTCOME_: "How a battle ended (battle.end).",
    BW_MOVE_SOURCE_: "How a Pokemon learns a move (move.learn).",
    BW_EVOLVE_: "How a Pokemon evolves (mon.evolve).",
    BW_HEAL_: "Where a heal came from (heal).",
    BW_EXP_: "Where experience comes from (mon.exp).",
    BW_LEVELUP_: "How a level was gained (mon.levelup).",
    BW_ITEM_FROM_: "Where an item comes from (item.add).",
    BW_MONEY_: "Where a money change comes from (money.change).",
    BW_REVIVE_: "What brought a fainted Pokemon back (mon.revive).",
    BW_CURE_: "Where a status cure comes from (mon.cure).",
    BW_PP_: "Where a PP restore comes from (mon.pp).",
    BW_FORM_: "Where a form change comes from (mon.form).",
    BW_HEAL_HP_: "Where an HP restore happens (heal.hp).",
    BW_SCRIPT_VM_: "The two script machines.",
    BW_SCRIPT_: "What a script handler answers.",
    BW_MON_FIELD_: "Pokemon field ids (party_get, party_set, box_get, box_set, battle_get, battle_set)",
    BW_SESSION_: "What a session handler is told about",
    BW_RESTART_: "The flags of a run restart",
  },

  // Per event: `when` replaces v4's sentence only where v4's is technical (these
  // go back to eidos); `facts` are details v4 does not carry; `functions` are
  // the calls the page links.
  events: {
    "battle.action": { facts: ["For a fight it fires several times a turn, once per move checked."] },
    "battle.faint": {
      facts: [
        "mon is a battle reference (place EIDOS_MON_BATTLE); side 0 is the player's. cause, cause_move and cause_damage are empty or 0 when the faint came from weather, a hazard or a status.",
        "A whiteout has its own event, whiteout. It is not the same moment as a single faint.",
        "Poison on the field never faints a Pokemon in this game (it stops at 1 HP).",
      ],
    },
    "mon.levelup": { facts: ["For a Rare Candy it fires after the level-up. To stop the candy, refuse item.use."] },
    "money.change": { facts: ["source says why: a script, a purchase, a sale or a battle. A handler may change the amount only in the direction of the call."] },
    heal: { facts: ["Refused, the whole heal does not happen: no member event fires. To spare one member, see mon.revive and heal.hp."] },
    "heal.hp": {
      when: "The game is about to add HP to a Pokemon that has HP. On the field it fires when the party screen restores HP with an item. In a battle it fires for every HP restore: a bag item, a berry, Leftovers, a move or an ability. A handler may change the amount, or refuse to add nothing. A party heal fires it once for each member that has lost HP, with source EIDOS_HEAL_HP_PARTY and the amount that fills it; refused, that member keeps the HP it had. A fainted Pokemon coming back is mon.revive.",
      facts: [
        "source is 0 (EIDOS_HEAL_HP_FIELD) on the field and 1 (EIDOS_HEAL_HP_BATTLE) in a battle. The amount is capped at max HP after the handler runs.",
        "On the field the item is used up even when the restore is refused: refuse item.use to keep it.",
      ],
      functions: ["party_get", "battle_get", "item_count", "message"],
    },
    "mon.revive": {
      when: "A fainted Pokemon is about to come back: a Revive, Max Revive, Revival Herb or Sacred Ash, a revive item in battle, or a party heal. A handler may change the HP it comes back with, or refuse and leave it fainted.",
      facts: [
        "source says what brought it back: EIDOS_REVIVE_FIELD_ITEM, _BATTLE_ITEM, _MOVE, _CENTRE, _POWER or _SCRIPT. The port reports every script heal as _SCRIPT, the Centre's too, because the nurse runs the same script command.",
        "hp is at least 1 and at most max_hp.",
      ],
      functions: ["party_get", "battle_get", "species_base", "message"],
    },
    "mon.cure": {
      when: "A status is about to be cured: by an item on the party screen, in a battle, or by a party heal. Refusing keeps the status.",
      facts: [
        "status is the game's own number: 1 paralysis, 2 sleep, 3 freeze, 4 burn, 5 poison. In a battle, 6 and up are the battle's own conditions, such as confusion.",
        "Refused on the party screen, the item is not used. Refused in a battle, the game's text may still say the status was cured.",
        "A party heal fires it only for members that have a status.",
      ],
      functions: ["party_get", "battle_get", "message"],
    },
    "mon.pp": {
      when: "A move is about to get PP back, from an item on the party screen or in a battle. A handler may change the amount. Refusing gives none.",
      facts: ["amount is the PP the move gets, before the cap at its maximum. move is 0 when the port cannot read it."],
      functions: ["party_get", "battle_get", "message"],
    },
    "mon.form": {
      when: "A Pokemon is about to change form, on the field or in a battle. A handler may change the new form. Refusing keeps the form it has.",
      functions: ["party_get", "battle_get", "species_base", "message"],
    },
    "battle.exp": {
      when: "The battle hands out experience after an opposing Pokemon fainted, as a whole, before mon.exp fires for each member that gains. A handler may scale every share. Refused, nobody gains and mon.exp does not fire.",
      facts: ["members has bit i set for each party position i that gains. scale is a percentage of every share: 100 leaves them. To change one member only, use mon.exp."],
      functions: ["party_get", "battle_get", "battle_wild"],
    },
    "battle.trainer_mon": {
      when: "Once for each Pokemon of the opposing trainer, in party order, after the game builds the party and before the battle starts. A rule may change the level, species, item, moves, ability and form. Refused, the member leaves the party before the battle; the last one always stays.",
    },
  },

  functions: {
    log: { what: "Writes a line to the port's mod log. Pass the text and its length.", tiers: ["c"] },
    on: { what: "Binds a handler to an event by the event's id. The handler is a function table index: in C, the handler's address cast to uint32_t. Call it from eidos_mod_start.", result: "0 when bound.", tiers: ["c"] },
    script: { what: "Binds a handler to a script command of one of the two script machines. vm is an EIDOS_SCRIPT_VM_* constant, opcode the command, and the handler a function table index. The handler returns an EIDOS_SCRIPT_* answer.", result: "0 when bound.", tiers: ["c"] },
    override: { what: "Binds a handler to a game function from the raw hook list, by region and address. The handler is a function table index.", tiers: ["c"] },
    rd8: { what: "Reads a byte of game memory at a DS address. The access is bounds-checked.", tiers: ["c"] },
    rd16: { what: "Reads 16 bits of game memory at a DS address.", tiers: ["c"] },
    rd32: { what: "Reads 32 bits of game memory at a DS address.", tiers: ["c"] },
    wr8: { what: "Writes a byte of game memory at a DS address. The access is bounds-checked.", tiers: ["c"] },
    wr16: { what: "Writes 16 bits of game memory at a DS address.", tiers: ["c"] },
    original: { what: "Inside a hook handler, calls the next link of the chain (the game's own function, or the next wrap) with the arguments in args, and returns its result. Pass the argument count in nargs.", result: "The result of the call, as a 64-bit value.", tiers: ["c"] },
    read: { what: "Copies len bytes of game memory from a DS address into the mod's memory.", result: "0 on success, non-zero when the range is out of bounds.", tiers: ["c"] },
    write: { what: "Copies len bytes of the mod's memory into game memory at a DS address.", result: "0 on success, non-zero when the range is out of bounds.", tiers: ["c"] },
    wr32: { what: "Writes 32 bits of game memory at a DS address.", tiers: ["c"] },

    party_count: { what: "How many Pokemon the party holds.", tiers: ["c"], rule: "party_count" },
    party_ref: { what: "A reference to the party Pokemon in a slot.", result: "0 on success.", tiers: ["c"] },
    party_get: { what: "Reads one field of a party Pokemon. Field ids are the EIDOS_MON_FIELD_* constants.", result: "0 on success; the value goes into out.", tiers: ["c"], rule: "species[m], level[m], hp[m], max_hp[m], type1[m], type2[m], held_item[m]" },
    party_set: { what: "Writes one field of a party Pokemon. During a battle the write does not reach the battle and is lost when the battle writes its results back.", result: "0 on success.", tiers: ["c"], rule: "set hp[m] value" },
    box_count: { what: "How many boxes are usable.", tiers: ["c"] },
    box_ref: { what: "A reference to the Pokemon in a box slot.", result: "0 on success.", tiers: ["c"] },
    box_get: { what: "Reads one field of a boxed Pokemon.", result: "0 on success.", tiers: ["c"] },
    box_set: { what: "Writes one field of a boxed Pokemon.", result: "0 on success.", tiers: ["c"] },
    battle_ref: { what: "A reference to the battler on a side and slot.", result: "0 on success.", tiers: ["c"] },
    battle_get: { what: "Reads one field of a Pokemon in battle. Use it, not party_get, while a battle runs.", result: "0 on success.", tiers: ["c"] },
    battle_set: { what: "Writes one field of a Pokemon in battle.", result: "0 on success.", tiers: ["c"] },
    species_value: { what: "Reads a personal parameter of a species: types, stats, catch rate.", result: "0 on success.", tiers: ["c"], rule: "species_type1[s], species_type2[s], species_bst[s]" },
    battle_trainer: { what: "The trainer id of the first opposing trainer of the battle in progress, 0 for none. battle_trainer2 is the second trainer of a double.", tiers: ["rules", "c"], rule: "battle_trainer, battle_trainer2" },
    species_base: { what: "The first stage of the evolution line of a species. Two species are in one line when their bases are equal.", result: "The base species, as a number.", tiers: ["rules", "c"], rule: "species_base[s]" },
    species_shiny: { what: "Whether a personality value is shiny for a species.", tiers: ["c"], rule: "shiny[m]" },
    dex_caught: { what: "Whether a species is registered as caught.", tiers: ["c"], rule: "caught[s]" },
    badge_count: { what: "How many badges the player holds.", tiers: ["c"], rule: "badges" },
    badge_has: { what: "Whether the player holds one badge.", tiers: ["c"], rule: "badge[n]" },
    money: { what: "The player's money.", tiers: ["c"], rule: "money" },
    money_set: { what: "Sets the player's money.", result: "0 on success.", tiers: ["c"], rule: "set money value, add money value, sub money value" },
    flag: { what: "Reads an event flag.", result: "0 on success; the value goes into out.", tiers: ["c"] },
    flag_set: { what: "Sets or clears an event flag.", result: "0 on success.", tiers: ["c"] },
    zone: { what: "The id of the zone the player is in.", tiers: ["c"], rule: "zone" },
    player_pos: { what: "The player's cell.", result: "0 on success.", tiers: ["c"] },
    battle_style: { what: "The battle style option: shift or set.", tiers: ["c"], rule: "battle_style" },
    mon_to_box: { what: "Moves a party Pokemon to the first free box slot. The port queues it and applies it when no battle, party screen or PC holds the party open.", result: "0 on success.", tiers: ["c"] },
    mon_take_item: { what: "Returns a Pokemon's held item to the bag. Queued like mon_to_box.", result: "0 on success.", tiers: ["c"] },
    mon_release: {
      what: "Releases a Pokemon of the player's party for good. It takes a party reference or a reference to one of the player's Pokemon in battle. The release is queued and applied at the first player step on the field after the battle; the Pokemon is found by its personality value then, so a switch or a compaction cannot hit the wrong one. It is refused when it would leave the party with no Pokemon that is not an egg. The party compacts and the next member becomes the lead.",
      result: "0 on success.",
      tiers: ["rules", "c"],
      rule: "do release event.mon",
    },
    item_count: {
      what: "How many of an item the bag holds.",
      result: "The count, or a negative number when there is no bag yet.",
      tiers: ["rules", "c"],
      rule: "item_count[i]",
    },
    item_add: {
      what: "Puts a number of an item in the bag at once.",
      result: "0 on success. Non-zero when the item is past the item limit, the count is 0 or the pocket is full; a refused add puts nothing in.",
      tiers: ["rules", "c"],
      rule: "do give item count",
    },

    message: { what: "Shows a line of text in the game's message window. It is queued and shown at the next point where the game accepts a message. Text is UTF-8; characters the font lacks show as ?.", tiers: ["rules", "c"], rule: "message \"text\"" },
    confirm: { what: "Asks a yes/no question in the field. In battle it answers 0 at once.", result: "1 for yes.", tiers: ["c"] },
    session_host: { what: "Starts a peer session through the relay and returns its code. It blocks for up to three seconds.", result: "0 on success.", tiers: ["c"] },
    session_join: { what: "Joins a session with its code. It blocks for up to three seconds.", result: "0 on success.", tiers: ["c"] },
    session_leave: { what: "Leaves the session.", tiers: ["c"] },
    session_send: { what: "Sends up to 4 KB to one peer, or to everyone with peer 0. Only the same mod on the other side receives it.", result: "0 on success, else a reason code.", tiers: ["c"], rule: "send value" },
    on_session: { what: "Sets the handler for peer messages, joins and leaves. It runs on the game frame.", tiers: ["c"] },
    avatar_create: { what: "Creates another player's avatar in the field.", result: "A handle, or 0 when there is no room.", tiers: ["c"] },
    avatar_move: { what: "Walks an avatar to a cell.", tiers: ["c"] },
    avatar_delete: { what: "Removes an avatar. Avatars are also deleted at each map change.", tiers: ["c"] },
    mon_export: { what: "Writes a Pokemon to a 252-byte buffer.", result: "Bytes written.", tiers: ["c"] },
    mon_import: { what: "Reads a Pokemon from a buffer into the party, or a box when the party is full.", result: "0 on success.", tiers: ["c"] },
    panel_set: { what: "Sets one of the 16 lines of the run-status panel beside the screens.", tiers: ["c"] },
    panel_clear: { what: "Clears the panel.", tiers: ["c"] },
    timer_start: { what: "Starts the mod's timer. It counts game time, not wall time.", tiers: ["c"] },
    timer_stop: { what: "Stops the timer.", tiers: ["c"] },
    timer_ms: { what: "The timer in milliseconds.", tiers: ["c"] },
    split: { what: "Records a named split; a run keeps up to 32 and writes them to a file.", tiers: ["c"] },
    run_restart: { what: "Restarts the game, optionally into a new game and with the save deleted.", tiers: ["c"] },
    rand_seed: { what: "Seeds the mod's own random source (SplitMix64). The same seed gives the same sequence on every platform.", tiers: ["c"] },
    rand_next: { what: "The next 32-bit value from the mod's random source.", tiers: ["c"] },
    on_command: { what: "Sets the handler for text commands from tools connected to the local control port.", tiers: ["c"] },
    publish: { what: "Sends a topic and a JSON line to every connected tool.", tiers: ["c"] },
    input_press: { what: "Holds DS buttons for a number of frames, as if the player pressed them.", tiers: ["c"] },
    on_frame: { what: "Sets a handler that runs once per game frame.", tiers: ["c"] },
    save_send: { what: "Sends the current save and the mod's saved data to a peer.", result: "0 when on its way.", tiers: ["c"] },
    on_save_received: { what: "Sets the handler for a save sent by a peer.", tiers: ["c"] },
    area_of: { what: "The area of a zone: the place name the game shows on entry. Every floor of a cave and every building of a town share one area.", result: "The area number, or a negative number when the zone data is not loaded.", tiers: ["rules", "c"], rule: "area_of[z]" },
    battle_wild: { what: "Whether the battle in progress is a wild one.", result: "1 for a wild battle, 0 for a trainer's, -1 when no battle runs.", tiers: ["rules", "c"], rule: "wild_battle" },
    play_seconds: { what: "The play time of the save in force, in seconds.", tiers: ["rules", "c"], rule: "play_seconds" },
    item_value: { what: "Reads one data field of an item, by the game's field id.", result: "0 on success; non-zero past the item limit.", tiers: ["rules", "c"], rule: "item_pocket[i]" },
    move_value: { what: "Reads one data field of a move, by the game's field id: type, damage class or power.", result: "0 on success; non-zero past the move limit.", tiers: ["rules", "c"], rule: "move_type[v], move_power[v], move_class[v]" },
    mon_to_box_n: { what: "Moves a party Pokemon to a named box (0 is the first). Queued like mon_to_box and applied at the next field step. It fails when the box is full, and it never leaves the party without a Pokemon that is not an egg.", result: "0 on success.", tiers: ["rules", "c"], rule: "do box event.mon n" },
    mon_from_box: { what: "Moves the Pokemon at a box and slot to the party. Queued like mon_to_box. It fails when the party is full or the slot is empty.", result: "0 on success.", tiers: ["rules", "c"], rule: "do withdraw box slot" },
    mon_recalc: { what: "Works out a party Pokemon's level and stats again from its species, experience, IVs and EVs, as the game does after a level up. Call it after party_set changes any of those. It does not work on a box or battle reference.", result: "0 on success.", tiers: ["c"] },
    table_get: { what: "Reads a value of one of the mod's own data tables (a `table` line in the manifest). A mod reads only its own tables.", result: "0 when the key is in the table, 1 when it is not (the table's default is written to out), -1 when the mod has no table with that name.", tiers: ["rules", "c"], rule: "table.name[key]" },
    setting_get: { what: "Reads one of the mod's own options (declared in the file named by the manifest's settings key). The key is the one on the option's line.", result: "0 or 1 for a bool, the index of the chosen name for a choice, the number for an int option, -1 for a key the mod does not declare.", tiers: ["rules", "c"], rule: "setting.key" },
    state_load: { what: "Reads the mod's own saved data, kept per save slot.", result: "Bytes read, -1 on damage.", tiers: ["c"] },
    state_store: { what: "Writes the mod's own saved data (up to 4 MiB).", result: "0 on success.", tiers: ["c"] },
    state_clear: { what: "Deletes the mod's saved data.", tiers: ["c"] },
    state_info: { what: "Reads the header of the mod's saved data.", tiers: ["c"] },
    state_dir: { what: "The path of the mod's own folder.", tiers: ["c"] },
    state_in_step: { what: "Whether the mod's saved data was written together with the game save now in force.", result: "1 in step, 0 out of step, -1 unknown.", tiers: ["c"] },
  },

  // One plain line for each rule read the port added in version 4. The state
  // page shows it instead of the engine's table row.
  // Rule state fields: v4 describes some fields together or not at all; these
  // say each field alone (they go back to eidos). The rest use v4's text.
  state: {
    hp: "The current HP of the Pokemon.",
    max_hp: "The maximum HP of the Pokemon.",
    type1: "The first type of the Pokemon.",
    type2: "The second type of the Pokemon.",
    species_type1: "The first type of species s.",
    species_type2: "The second type of species s.",
    personality: "The personality value of the Pokemon.",
    ot_id: "The id of the trainer who first had the Pokemon.",
    exp: "The experience points the Pokemon holds.",
    friendship: "The friendship value of the Pokemon.",
    ability: "The id of the Pokemon's ability.",
    move1: "The id of the Pokemon's first move.",
    move2: "The id of the Pokemon's second move.",
    move3: "The id of the Pokemon's third move.",
    move4: "The id of the Pokemon's fourth move.",
    is_egg: "1 when the Pokemon is an egg.",
    gender: "The Pokemon's gender, as the game stores it.",
    form: "The Pokemon's form number.",
    nature: "The id of the Pokemon's nature.",
    has_nickname: "1 when the Pokemon has a nickname.",
    met_location: "The id of the place where the Pokemon was met.",
    met_level: "The level the Pokemon had when it was met.",
    origin_game: "The id of the game the Pokemon comes from.",
    fateful: "1 when the Pokemon was met in a fateful encounter.",
    ball: "The id of the ball the Pokemon was caught in.",
    place: "Where a reference points: the EIDOS_MON_* place id (party, box or battle).",
    side: "In a battle, the side a reference belongs to: 0 is the player's.",
    species_stage: "The evolution stage of species s: 0 for the first stage of a line.",
    catch_rate: "The catch rate of species s.",
    battle_trainer: "The trainer id of the first opposing trainer of the battle in progress, 0 for none.",
    battle_trainer2: "The trainer id of the second opposing trainer, 0 for none.",
    gender_ratio: "The gender ratio byte of species s.",
    growth: "The growth rate id of species s.",
    egg_group1: "The first egg group of species s.",
    egg_group2: "The second egg group of species s.",
    party_max_level: "The highest level in the party, eggs left out.",
    party_alive: "How many party Pokemon have HP left, eggs left out.",
    move_type: "The type id of move v.",
    move_power: "The power of move v.",
    move_class: "The damage class of move v.",
  },

  functionGroups: [
    { title: "Entry points", fns: ["log","on","script","override","original"] },
    { title: "Game memory", fns: ["rd8","rd16","rd32","wr8","wr16","wr32","read","write"] },
    { title: "The party", fns: ["party_count","party_ref","party_get","party_set"] },
    { title: "The boxes", fns: ["box_count","box_ref","box_get","box_set"] },
    { title: "A Pokemon in battle", fns: ["battle_ref","battle_get","battle_set"] },
    { title: "Species and dex", fns: ["species_value","species_base","species_shiny","dex_caught"] },
    { title: "Badges, money and flags", fns: ["badge_count","badge_has","money","money_set","flag","flag_set"] },
    { title: "The map and the player", fns: ["zone"] },
    { title: "Moves between places", fns: ["mon_to_box","mon_to_box_n","mon_from_box","mon_take_item","mon_release"] },
    { title: "Recalculating a Pokemon", fns: ["mon_recalc"] },
    { title: "The world, the battle and play time", fns: ["area_of","battle_wild","battle_trainer","play_seconds"] },
    { title: "Item and move data", fns: ["item_value","move_value"] },
    { title: "Mod data tables", fns: ["table_get"] },
    { title: "Messages", fns: ["message","confirm"] },
    { title: "Miscellaneous", fns: ["item_count","item_add","setting_get","player_pos","battle_style"] },
    { title: "Peer sessions", fns: ["session_host","session_join","session_leave","session_send","on_session"] },
    { title: "Field avatars", fns: ["avatar_create","avatar_move","avatar_delete"] },
    { title: "Pokemon export and import", fns: ["mon_export","mon_import"] },
    { title: "Status panel, timer and splits", fns: ["panel_set","panel_clear","timer_start","timer_stop","timer_ms","split"] },
    { title: "Run, random, input and frame", fns: ["run_restart","rand_seed","rand_next","input_press","on_frame"] },
    { title: "External control and save handoff", fns: ["on_command","publish","save_send","on_save_received"] },
    { title: "Saved data", fns: ["state_load","state_store","state_clear","state_info","state_dir","state_in_step"] },
  ],

  actions: [
    { name: "refuse", syntax: "do refuse", what: "Refuses the event. Valid only on events that can be refused; the game then takes the event's refused outcome.", example: "do refuse" },
    { name: "set", syntax: "do set <target> <expr>", what: "Assigns a payload field marked writable, a writable state field, or a mod cell.", example: "do set event.amount 0" },
    { name: "add", syntax: "do add <target> <expr>", what: "Adds to a target.", example: "do add money 500" },
    { name: "sub", syntax: "do sub <target> <expr>", what: "Subtracts from a target.", example: "do sub money 1000" },
    { name: "message", syntax: "do message <text>", what: "Shows a line of text to the player.", example: "do message \"Be careful.\"" },
    { name: "send", syntax: "do send <expr>", what: "Sends a value to the peers of the current session.", example: "do send event.species" },
    { name: "release", syntax: "do release <mon or party position>", what: "Releases a Pokemon of the player's party for good. It is applied at the first player step on the field after a battle, and not when it would leave only eggs.", example: "do release event.mon" },
    { name: "box", syntax: "do box <mon or party position> <n>", what: "Moves a party Pokemon to box n, counted from 0. It is applied at the next field step. It fails when the box is full, and not when it would leave only eggs.", example: "do box event.mon 23" },
    { name: "withdraw", syntax: "do withdraw <box> <slot>", what: "Moves the Pokemon at a box and slot to the party. It is applied at the next field step. It fails when the party is full or the slot is empty.", example: "do withdraw 0 1" },
    { name: "give", syntax: "do give <item> <count>", what: "Puts a number of an item in the bag at once. Nothing happens when the bag cannot take it, and the rule stops for that firing.", example: "do give 17 5" },
  ],

  pages: [
    {
      slug: "code-mods",
      section: "functions",
      title: "Code mods at a glance",
      blocks: [
        { t: "p", text: "A code mod is C built to WebAssembly. It exports eidos_mod_start and eidos_mod_stop, binds handlers to events with `eidos_on`, and reaches the game only through the calls of eidos_wasm.h. It cannot call game functions directly; raw hooks replace or wrap them." },
        { t: "p", text: "A handler gets a copy of the event payload and returns EIDOS_EVENT_CONTINUE or EIDOS_EVENT_REFUSE. It may write the payload fields marked writable; the port reads them back before the game goes on. A write to any other field has no effect. A refusal at an event that cannot be refused is ignored and logged once." },
        { t: "list", items: ["The snippets in these docs build as they are with the Code view's template: `#include \"eidos_wasm.h\"`, the constants are spelled EIDOS_, and the payload structs are Eidos<Name>.", "A handler is registered by its function table index. In C that is its address: `eidos_on(EIDOS_EVENT_MON_EXP, (uint32_t)(uintptr_t)on_exp)`.", "A Pokemon reference (EidosMonRef) goes to a state call as its five fields: handle, place, side, box and slot.", "No libc and no WASI: a mod reaches nothing outside the game except through the port's imports. The template brings a small allocator and the memory functions."] },
        { t: "p", text: "Several handlers on one event all run, in load order. Writes chain and any refusal refuses. The battle works on a copy of each Pokemon: during a battle read and write the battle record (battle_get, battle_set), not the party record." },
      ],
    },
    {
      slug: "overview",
      title: "Rules at a glance",
      blocks: [
        { t: "p", text: "A rule acts on one event without any code. It has an event, zero or more guards (`when`) and one or more actions (`do`). It has no loops, functions, local variables or branches." },
        { t: "p", text: "A rule file holds one rule. The manifest binds the file to its event with a `rule = <event> <path>` line; the event in the file must match." },
        { t: "code", lang: "rule", text: "language 1\nrule faint_penalty on battle.faint\n    when event.side == 0\n    do sub money 1000\n" },
        { t: "list", items: ["Several `when` lines must all hold.", "A rule that reaches 256 steps stops for that firing without refusing.", "Division or modulo by zero gives 0. An unset cell reads 0.", "Several mods on one event all run, in load order. Writes chain and any refusal refuses."] },
        { t: "p", text: "Need a loop, a search of the party, a fallback or anything beyond this? Write a code mod; the two mix in one mod." },
      ],
    },
    {
      slug: "expressions",
      title: "Names and expressions",
      blocks: [
        { t: "p", text: "Read a payload field as `event.<field>`. Read the game through state fields such as `money` or `level[event.mon]`. Keep your own data in `state.<cell>`; with a subscript it is a keyed table, up to 1024 keys, saved with the game." },
        { t: "table", head: ["Type", "Values"], rows: [["int", "A 32-bit signed number. Every numeric payload and state field."], ["bool", "true or false. Comparisons and flag fields."], ["text", "A string literal."], ["mon", "A Pokemon reference. Use it as a subscript: hp[event.mon]."]] },
        { t: "p", text: "Operators, loosest to tightest: `||`, `&&`, `!`, comparisons (`== != < <= > >=`), `+ -`, `* / %`, unary `-`. Both sides of a comparison must have the same type; arithmetic needs int. There are no names for enum values: use the numbers listed on the event page." },
        { t: "p", text: "Read the mod's own options as `setting.<key>` (see Options) and its data tables as `table.<name>[key]`." },
        { t: "p", text: "Operators are read as two characters at most. A pair that is not an operator reads as two, so `x*-1` and `a<-1` mean what they look like." },
      ],
    },
    {
      slug: "targets",
      title: "What a rule can write",
      blocks: [
        { t: "table", head: ["Target", "Writable when"], rows: [["event.<field>", "The payload page marks the field writable. Any other field is a load error."], ["money, hp[m], battle_style", "The state field page says the port can write it."], ["state.<cell>, state.<cell>[key]", "Always. The cell belongs to your mod."]] },
        { t: "p", text: "The game's later write wins. To make a change stick, write the payload field of the event the game derives the value from, not the state field after it." },
      ],
    },
    {
      slug: "tables",
      title: "Data tables",
      blocks: [
        { t: "p", text: "A table is a file of numbers the mod reads by key: a level cap for each badge count, a list of banned species, a zone-to-area map. Rules and code read it. What the table means is up to the mod." },
        { t: "p", text: "Declare each table with one line in the manifest. The Tables view writes it for you." },
        { t: "code", lang: "manifest", text: "table = caps tables/caps.txt\n" },
        { t: "list", items: ["The name is letters, digits and `_`, does not start with a digit, and has at most 31 characters.", "The path is inside the mod folder.", "A mod can hold up to 128 tables in all, counting every loaded mod.", "A table belongs to the mod that declares it. Two mods can each have a table called `caps`; neither reads the other's."] },
        { t: "p", text: "The file has one entry per line. A key and a value are signed 32-bit numbers, written in decimal or with `0x` for hex. `#` starts a comment. `default = N` is the value of every key the table does not list, and 0 when there is no such line." },
        { t: "code", lang: "table", text: "# level cap per badge count\n0 = 12\n1 = 20     # after the first gym\n0x2 = 25\ndefault = 100\n" },
        { t: "list", items: ["When a key is listed twice, the later line wins. The checker warns about it.", "A table holds up to 65536 entries, and the file is at most 16 MiB.", "A table that fails to load is reported when the game starts. The mod's other tables and rules still load."] },
        { t: "p", text: "A rule reads a table with `table.<name>[key]`. The key is any number expression, such as `badges`. A key the table lacks reads the default." },
        { t: "code", lang: "rule", text: "language 1\nrule level_cap on mon.exp\n    when level[event.mon] >= table.caps[badges]\n    do set event.amount 0\n" },
        { t: "p", text: "A rule that names a table the mod does not declare is refused when the game loads it. The checker reports the same error. Code reads a table with `table_get(name, key, &out)`." },
      ],
    },
    {
      slug: "options",
      title: "Options",
      blocks: [
        { t: "p", text: "A mod can show options on the game's settings screen, under its own name. The player picks a value, the game keeps it beside the mod's saved data, and the mod reads it. Declare the options in a file and name it in the manifest. The Options tab of Mod settings writes both." },
        { t: "code", lang: "manifest", text: "settings = options.txt\n" },
        { t: "p", text: "The file has one option per line: `option <key> <kind> \"<label>\" <value>`. `#` starts a comment, except inside the quoted label. A line that does not start with `option` is skipped." },
        { t: "code", lang: "options", text: "option dupes   bool    \"Dupes clause\"    on\noption cap     choice  \"Level caps\"      off|gym|ace\noption lives   int     \"Lives\"           0 10 3\n" },
        { t: "table", head: ["Kind", "Value", "Reads as"], rows: [["bool", "`on`, `off`, `true`, `false`, `1` or `0`. Any other text is reported and the option is ignored.", "1 or 0"], ["choice", "Names split by `|`, 31 characters each, as many as you want. The first is the default.", "The index of the chosen name"], ["int", "`<min> <max> <default>`, whole numbers within a billion of zero, with min <= default <= max. The screen steps by one.", "The number"]] },
        { t: "list", items: ["The key is one word. It cannot hold a space, `=`, `#` or a quote, and it has at most 63 characters. Dots are allowed.", "The label is quoted, has no quote, and has at most 63 characters.", "A mod can declare any number of options.", "A line the game cannot read is reported and ignored. The checker reports it too.", "When a key is declared twice, the game reports it and ignores the second."] },
        { t: "p", text: "A rule reads an option with `setting.<key>`. A rule reads a dotted key as written: `setting.nuzlocke.dupes`. Each word of a key is letters, digits and `_`, not starting with a digit. A key the mod does not declare reads -1, and the checker warns about it. A mod with no namespace has no settings, and the game refuses a rule that reads one. Code reads an option with `setting_get(key)`." },
        { t: "code", lang: "rule", text: "language 1\nrule lives on catch.attempt\n    when setting.lives == 0\n    do refuse\n" },
      ],
    },
  ],

  hookPage: {
    title: "Raw hooks",
    blocks: [
      { t: "p", text: "A raw hook replaces or wraps one game function from the list below. It is for code mods only, with the game's own arguments. Prefer events and rules; use a hook when no event covers what you need." },
      { t: "p", text: "In kenon, one hook is one line in the Hooks view: `region:address mode symbol`. The symbol is a function exported by your mod's code." },
      { t: "code", lang: "hooks", text: "ov093:0x021CBD48 wrap my_catch_rate sig=\"int (void)\" switch=my_catch\n" },
      { t: "list", items: ["`wrap` runs your function in place of the game's and lets you call the original through the chain. Several wraps nest, the last loaded runs first.", "`replace` takes the function for itself. When two mods replace one function, the later mod wins and the other is reported as shadowed.", "`sig` states the prototype you expect, as `\"<return> (<parameters>)\"`. The port refuses the hook when it differs from the function's own.", "A hook is keyed by region and address, never by name, because names change when the decompilation is cleaned up.", "The game's arguments can change when the code is restructured. A mod with raw hooks states the port version it was built for."] },
      { t: "p", text: "Rows with events are already bound to a stable event: that event fires from the same function." },
    ],
  },

  // Families are read from the function name's prefix. They say where a
  // function lives, not what it does.
  hookFamilies: [
    ["SAVE_CONTROL_", "Save system"],
    ["BOX_SAVEDATA_", "Box save data"],
    ["MYITEM_", "Bag save data"],
    ["BEACON_", "Money, badges and counters"],
    ["ZUKAN_", "Pokedex"],
    ["GPOWER_", "Gear powers"],
    ["GAME_DATA_", "Game data"],
    ["GAME_COMM_", "Wireless communication"],
    ["BTL_SETUP_", "Battle setup"],
    ["BTL_SERVER_", "Battle server"],
    ["BTL_CLIENT_", "Battle client and menus"],
    ["BTL_POKEPARAM_", "Battle Pokemon"],
    ["BTL_", "Battle"],
    ["BTLV_", "Battle visuals"],
    ["ZONEDATA_", "Zone data"],
    ["POKE_TOOL_", "Pokemon data"],
    ["POKEPARTY_", "Party"],
    ["SHINKA_", "Evolution"],
    ["DWC", "Wi-Fi library"],
    ["WM", "Wireless library"],
    ["FS_", "Files and overlays"],
    ["SCRCMD_", "Script commands"],
    ["SCRIPT_", "Script system"],
    ["EVENT_", "Field events"],
    ["FIELD_ENCOUNT_", "Wild and trainer encounters"],
    ["FIELD_", "Field"],
    ["FIELDMAP_", "Field map"],
    ["BOX2_", "PC box screen"],
    ["POKELIST_", "Party screen"],
    ["PLIST_", "Party screen"],
    ["SODATEYA_", "Day Care"],
    ["MOVE_POKEMON_", "Field moves"],
    ["EVENTDATA_", "Event data"],
    ["POKE_SUMMARY_", "Summary screen"],
    ["FLD_TRADE_", "In-game trades"],
    ["TRCARD_", "Trainer card"],
    ["WCM_", "Wireless library"],
    ["SOCL_", "Wireless library"],
    ["ITEMMENU_", "Item menu"],
    ["GTSNEGO_", "Global Trade Station"],
  ],

  recipes: [
    {
      slug: "faint-penalty",
      title: "Lose money when a Pokemon faints",
      goal: "Take 1000 money each time one of your Pokemon faints.",
      tier: ["rules"],
      events: ["battle.faint"],
      functions: [],
      steps: ["`battle.faint` fires when a Pokemon faints. `event.side` is 0 for the player's side.", "The rule guards on the side and subtracts from `money`."],
      rule: rule(`
language 1
rule faint_penalty on battle.faint
    when event.side == 0
    do sub money 1000
`),
    },
    {
      slug: "level-cap",
      title: "Cap the level",
      goal: "Stop a Pokemon from gaining experience once it reaches level 30.",
      tier: ["rules"],
      events: ["mon.exp"],
      functions: [],
      steps: ["`mon.exp` fires before experience is added. `event.level` is the level before; `event.amount` is writable.", "Setting the amount to 0 in the rule keeps the battle, the display and the saved Pokemon in agreement."],
      rule: rule(`
language 1
rule level_cap on mon.exp
    when event.level >= 30
    do set event.amount 0
`),
      notes: ["Refusing `mon.levelup` instead would leave the screen showing a level the Pokemon does not have; lower the experience."],
    },
    {
      slug: "block-wild-encounter",
      title: "Block grass encounters before the first badge",
      goal: "No wild battle in grass while the player has no badge.",
      tier: ["rules"],
      events: ["encounter.wild"],
      functions: [],
      steps: ["`encounter.wild` can be refused. `event.method` is 0 for grass.", "`badges` is a state field with the number of badges.", "A refused encounter is no battle; the player keeps walking."],
      rule: rule(`
language 1
rule no_early_grass on encounter.wild
    when badges == 0 && event.method == 0
    do refuse
`),
    },
    {
      slug: "catch-rate",
      title: "Make low-level Pokemon easier to catch",
      goal: "Double the catch rate for wild Pokemon below level 20.",
      tier: ["rules"],
      events: ["catch.attempt"],
      functions: [],
      steps: ["`catch.attempt` fires when a ball is thrown. `event.level` is the target's level.", "`event.rate` is the catch rate as a percentage of the game's own: 100 leaves it alone, 200 doubles it. It is a byte, so 255 is the most."],
      rule: rule(`
language 1
rule easy_catch on catch.attempt
    when event.level < 20
    do set event.rate 200
`),
    },
    {
      slug: "level-up-message",
      title: "Show a message on a level-up",
      goal: "Tell the player when a Pokemon reaches a level that is a multiple of 10.",
      tier: ["rules"],
      events: ["mon.levelup"],
      functions: ["message"],
      steps: ["`mon.levelup` fires once for the level crossing. `event.to` is the level reached.", "`message` shows text in the game's window. The text is a fixed string."],
      rule: rule(`
language 1
rule round_level on mon.levelup
    when event.to % 10 == 0
    do message "A round number!"
`),
    },
    {
      slug: "trainer-win-bonus",
      title: "Pay a bonus for beating a trainer",
      goal: "Add 500 money when the player wins a trainer battle.",
      tier: ["rules"],
      events: ["battle.end"],
      functions: ["money_set"],
      steps: ["`battle.end` carries `outcome` (0 is a win) and `wild` (0 for a trainer battle).", "The rule adds to `money`."],
      rule: rule(`
language 1
rule trainer_bonus on battle.end
    when event.outcome == 0 && event.wild == 0
    do add money 500
`),
      notes: ["`money.change` does not say where money came from, so a rule that doubles prize money there would also double other income. Test before relying on it."],
    },
    {
      slug: "force-shiny-fishing",
      title: "Every fished Pokemon is shiny",
      goal: "Make fishing encounters shiny.",
      tier: ["rules"],
      events: ["encounter.wild"],
      functions: [],
      steps: ["`encounter.wild` lets a rule write `event.shiny`: 1 forces shiny, 0 leaves the roll.", "`event.method` is 3 for fishing."],
      rule: rule(`
language 1
rule shiny_fishing on encounter.wild
    when event.method == 3
    do set event.shiny 1
`),
    },
    {
      slug: "no-pc-release",
      title: "Forbid releasing Pokemon from the PC",
      goal: "The release step does nothing and tells the player why.",
      tier: ["rules"],
      events: ["mon.release"],
      functions: ["message"],
      steps: ["`mon.release` fires on the release step of the box screen and can be refused. The Pokemon stays; the box redraws it.", "A refusal shows nothing by itself: add a `message`."],
      rule: rule(`
language 1
rule keep_everyone on mon.release
    do refuse
    do message "These Pokemon stay."
`),
    },
    {
      slug: "one-catch-per-route",
      title: "One catch per route",
      goal: "Allow one catch on each zone, then refuse more balls there.",
      tier: ["rules"],
      events: ["catch.result", "catch.attempt"],
      functions: [],
      steps: ["Two rules share a keyed cell: `state.route_used[zone]`. Cells are saved with the game.", "`catch.result` sets the cell when `event.caught` is 1.", "`catch.attempt` can be refused: the ball stays in the bag and the turn is spent.", "A rule file holds one rule, so these are two files."],
      rule: rule(`
language 1
rule mark_route_used on catch.result
    when event.caught == 1
    do set state.route_used[zone] 1
`),
      rule2: rule(`
language 1
rule one_catch_per_route on catch.attempt
    when state.route_used[zone] == 1
    do refuse
    do message "You already caught a Pokemon on this route."
`),
      notes: ["The first block is the rule for `catch.result`; the second is the rule for `catch.attempt`."],
    },
    {
      slug: "setting-toggle",
      title: "A setting that switches a feature",
      goal: "Halve all experience when the player turns on an option in the port's settings screen.",
      tier: ["rules", "c"],
      events: ["mon.exp"],
      functions: ["setting_get", "on"],
      steps: [
        "Declare the option in an options file and name it in the manifest with `settings = options.txt`. The Options tab of Mod settings writes both.",
        "A rule reads the option as `setting.<key>`. It is 0 or 1 for a bool, the index for a choice.",
        "In C, the handler for `mon.exp` reads the option with `setting_get`.",
        "`amount` is writable, so both halve it before the game uses it.",
      ],
      rule: rule(`
language 1
rule slow_exp on mon.exp
    when setting.slow_exp == 1
    do set event.amount event.amount / 2
`),
      settings: `option slow_exp bool "Slow experience" off\n`,
      c: `#include "eidos_wasm.h"

static int32_t on_exp(EidosEvent *event)
{
    EidosMonExp *e = (EidosMonExp *)event;
    if (eidos_setting_get("slow_exp") == 1) {
        e->amount /= 2;
    }
    return EIDOS_EVENT_CONTINUE;
}

${start("    return eidos_on(EIDOS_EVENT_MON_EXP, (uint32_t)(uintptr_t)on_exp);")}`,
    },
    {
      slug: "release-on-faint",
      title: "Release a Pokemon when it faints",
      goal: "A fainted Pokemon of the player is released for good (a Nuzlocke rule).",
      tier: ["rules", "c"],
      events: ["battle.faint"],
      functions: ["mon_release", "message"],
      steps: [
        "`battle.faint` fires when a Pokemon faints. `event.side` is 0 for the player's side and `event.mon` is the fainted Pokemon.",
        "`release` (in C, `mon_release`) queues the release. It is applied at the first player step on the field after the battle, so the battle is not disturbed.",
        "It does nothing if the Pokemon is the last one that is not an egg.",
      ],
      rule: rule(`
language 1
rule nuzlocke on battle.faint
    when event.side == 0
    do release event.mon
`),
      c: `#include "eidos_wasm.h"

static int32_t on_faint(EidosEvent *event)
{
    EidosBattleFaint *e = (EidosBattleFaint *)event;
    if (e->side == 0) {
        eidos_mon_release(${mon("e->mon")});
    }
    return EIDOS_EVENT_CONTINUE;
}

${start("    return eidos_on(EIDOS_EVENT_BATTLE_FAINT, (uint32_t)(uintptr_t)on_faint);")}`,
      notes: ["Whiteout is a separate event, `whiteout`. Poison on the field does not faint a Pokemon in this game."],
    },
    {
      slug: "gift-on-map",
      title: "Give an item on entering a map",
      goal: "Give 5 Potions the first time the player enters a zone.",
      tier: ["rules", "c"],
      events: ["route.change"],
      functions: ["item_add", "item_count"],
      steps: [
        "`route.change` fires on every map change; `event.zone` is the zone entered. Replace 115 with the zone you want.",
        "`give <item> <count>` puts the items in the bag at once. If the bag cannot take them, the rule stops for that firing and tries again on the next entry.",
        "The cell `state.gift_given` is saved with the game and reads 0 until it is set, so the gift is given once. The `give` line comes first, so the cell is set only after a successful gift.",
      ],
      rule: rule(`
language 1
rule potion_gift on route.change
    when event.zone == 115 && state.gift_given == 0
    do give 17 5
    do set state.gift_given 1
`),
      c: `#include "eidos_wasm.h"

static int g_given;

static int32_t on_route(EidosEvent *event)
{
    EidosRouteChange *e = (EidosRouteChange *)event;
    if (e->zone == 115 && !g_given && eidos_item_add(17, 5) == 0) {
        g_given = 1;
    }
    return EIDOS_EVENT_CONTINUE;
}

${start("    return eidos_on(EIDOS_EVENT_ROUTE_CHANGE, (uint32_t)(uintptr_t)on_route);")}`,
      notes: ["The C version keeps its flag in memory only. Save it with `state_store` to keep it across runs. `item_count[17]` reads the Potions the bag holds, for a rule that gives only when the bag has none."],
    },
    {
      slug: "halve-healing",
      title: "Halve all healing",
      goal: "Every HP restore gives half the HP, on the field and in battle.",
      tier: ["rules"],
      events: ["heal.hp"],
      functions: [],
      steps: [
        "`heal.hp` fires before HP is added. `event.amount` is the HP to add and is writable.",
        "The amount is capped at max HP after the rule runs.",
        "`event.source` is 0 on the field and 1 in battle, so a `when` line can limit the rule to one of them.",
      ],
      rule: rule(`
language 1
rule half_healing on heal.hp
    do set event.amount event.amount / 2
`),
    },
    {
      slug: "no-battle-healing",
      title: "No HP restores in battle",
      goal: "No Pokemon gets HP back during a battle, from any source.",
      tier: ["rules"],
      events: ["heal.hp"],
      functions: [],
      steps: [
        "`heal.hp` can be refused; a refusal adds no HP. In battle it fires for every restore: bag items, berries, Leftovers, moves and abilities.",
        "`event.source` is 1 for a battle.",
      ],
      rule: rule(`
language 1
rule no_battle_healing on heal.hp
    when event.source == 1
    do refuse
`),
      notes: ["A bag item is still used up when its restore is refused. To stop the item from being used, refuse `battle.item`: the player returns to the action menu."],
    },
    {
      slug: "faint-counter",
      title: "Count faints in the status panel",
      goal: "Show how many of your Pokemon fainted this session beside the screens.",
      tier: ["c"],
      events: ["battle.faint"],
      functions: ["panel_set", "on"],
      steps: ["`battle.faint` fires for both sides; `side` 0 is the player's.", "`panel_set` writes one of the 16 panel lines."],
      c: `#include "eidos_wasm.h"

static uint32_t g_faints;

static int32_t on_faint(EidosEvent *event)
{
    EidosBattleFaint *e = (EidosBattleFaint *)event;
    char line[24] = "Faints: ";
    char digits[11];
    uint32_t n = 0, v, i;
    if (e->side != 0) {
        return EIDOS_EVENT_CONTINUE;
    }
    g_faints++;
    v = g_faints;
    do {
        digits[n++] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    for (i = 0; i < n; i++) {
        line[8 + i] = digits[n - 1 - i];
    }
    line[8 + n] = 0;
    eidos_panel_set(0, line);
    return EIDOS_EVENT_CONTINUE;
}

${start("    return eidos_on(EIDOS_EVENT_BATTLE_FAINT, (uint32_t)(uintptr_t)on_faint);")}`,
    },
    {
      slug: "level-cap-badge",
      title: "Level cap by badge",
      goal: "A Pokemon gains no experience once it reaches the level cap for the badges the player holds.",
      tier: ["rules", "c"],
      events: ["mon.exp"],
      functions: ["table_get", "badge_count"],
      steps: [
        "Make a table called `caps` in the Tables view. Each key is a badge count and each value the highest level allowed. `default` covers every count the table does not list.",
        "`mon.exp` fires once for each party member that gains experience. Setting `event.amount` to 0 gives that member none.",
        "`table.caps[badges]` reads the cap for the badges the player holds now.",
      ],
      tables: [{ file: "tables/caps.txt", text: "# badge count = level cap\n0 = 12\n1 = 20\n2 = 25\n3 = 30\n4 = 36\n5 = 42\n6 = 48\n7 = 54\n8 = 60\ndefault = 100\n" }],
      rule: rule(`
language 1
rule level_cap on mon.exp
    when level[event.mon] >= table.caps[badges]
    do set event.amount 0
`),
      c: `#include "eidos_wasm.h"

static int32_t on_exp(EidosEvent *event)
{
    EidosMonExp *e = (EidosMonExp *)event;
    int32_t cap = 100;
    eidos_table_get("caps", (int32_t)eidos_badge_count(), &cap);
    if ((int32_t)e->level >= cap) {
        e->amount = 0;
    }
    return EIDOS_EVENT_CONTINUE;
}

${start("    return eidos_on(EIDOS_EVENT_MON_EXP, (uint32_t)(uintptr_t)on_exp);")}`,
      notes: ["The cap compares the level before the gain, as `event.level` and `level[event.mon]` both read it."],
    },
    {
      slug: "rare-candy-cap",
      title: "Rare Candy stops at the cap",
      goal: "A Rare Candy cannot be used on a Pokemon that has reached the level cap.",
      tier: ["rules"],
      events: ["item.use"],
      functions: ["message"],
      steps: [
        "`item.use` fires when an item is used and can be refused. A refusal keeps the Rare Candy in the bag and the level as it was.",
        "`event.item` is the item and `event.target` the Pokemon it is used on. Item 50 is the Rare Candy in the game's item list.",
        "The cap comes from the `caps` table of the level cap recipe.",
      ],
      tables: [{ file: "tables/caps.txt", text: "# badge count = level cap\n0 = 12\n1 = 20\n2 = 25\ndefault = 100\n" }],
      rule: rule(`
language 1
rule candy_cap on item.use
    when event.item == 50 && level[event.target] >= table.caps[badges]
    do refuse
    do message "It cannot grow past the cap yet."
`),
    },
    {
      slug: "dupes-clause",
      title: "Roll again for a species the player owns",
      goal: "A wild encounter of a species the player already owns is rolled again.",
      tier: ["rules"],
      events: ["encounter.wild"],
      functions: [],
      steps: [
        "`owned[s]` is 1 when the party or a box holds a Pokemon of species `s` that is not an egg.",
        "`event.reroll` is writable. Setting it to 1 rolls the encounter again with the game's own roll and fires the event again, at most 16 times. The last roll stands.",
        "A rule that refuses the event on a reroll leaves no encounter.",
      ],
      rule: rule(`
language 1
rule dupes on encounter.wild
    when owned[event.species] == 1
    do set event.reroll 1
`),
      notes: ["To skip only the first stage of an evolution line, add `species_stage[event.species] == 0` to the guard. To count a whole evolution line as one species, see the recipe on evolution lines."],
    },
    {
      slug: "dupes-by-line",
      title: "Dupes clause by evolution line",
      goal: "A wild encounter is rolled again when the player already has a Pokemon of the same evolution line in the party, or any stage of it in a box.",
      tier: ["rules", "c"],
      events: ["encounter.wild"],
      functions: ["species_base", "party_ref", "box_ref", "party_get", "box_get"],
      steps: [
        "`species_base[s]` is the first stage of the evolution line of species `s`. Two species are in one line when their bases are equal, so Charmander, Charmeleon and Charizard share a base.",
        "A rule compares the base of the rolled species with the base of each party member. A rule has no loops, so the guard lists the six party positions. `species[k]` with a number `k` reads the Pokemon in party position `k`, and `party_count` keeps the empty positions out.",
        "A rule cannot read the boxes one by one. The code version below checks the party and every box.",
        "`event.reroll` rolls the encounter again; the last roll stands.",
      ],
      rule: rule(`
language 1
rule dupes_by_line on encounter.wild
    when party_count > 0 && species_base[species[0]] == species_base[event.species] || party_count > 1 && species_base[species[1]] == species_base[event.species] || party_count > 2 && species_base[species[2]] == species_base[event.species] || party_count > 3 && species_base[species[3]] == species_base[event.species] || party_count > 4 && species_base[species[4]] == species_base[event.species] || party_count > 5 && species_base[species[5]] == species_base[event.species]
    do set event.reroll 1
`),
      c: `#include "eidos_wasm.h"

/* 1 when the Pokemon is in the line with this base and is not an egg. */
static int in_line(const EidosMonRef *ref, int boxed, int32_t base)
{
    uint32_t species = 0;
    uint32_t egg = 0;
    int32_t (*get)(uint32_t, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t *) =
        boxed ? eidos_box_get : eidos_party_get;
    if (get(${mon("ref", "->")}, EIDOS_MON_FIELD_SPECIES, &species) != 0) {
        return 0;
    }
    get(${mon("ref", "->")}, EIDOS_MON_FIELD_IS_EGG, &egg);
    return !egg && eidos_species_base(species) == base;
}

static int32_t on_wild(EidosEvent *event)
{
    EidosEncounterWild *e = (EidosEncounterWild *)event;
    int32_t base = eidos_species_base(e->species);
    EidosMonRef ref;
    uint32_t i, b, s;
    for (i = 0; i < eidos_party_count(); i++) {
        if (eidos_party_ref(i, &ref) == 0 && in_line(&ref, 0, base)) {
            e->reroll = 1;
            return EIDOS_EVENT_CONTINUE;
        }
    }
    for (b = 0; b < eidos_box_count(); b++) {
        for (s = 0; s < 30; s++) {
            if (eidos_box_ref(b, s, &ref) == 0 && in_line(&ref, 1, base)) {
                e->reroll = 1;
                return EIDOS_EVENT_CONTINUE;
            }
        }
    }
    return EIDOS_EVENT_CONTINUE;
}

${start("    return eidos_on(EIDOS_EVENT_ENCOUNTER_WILD, (uint32_t)(uintptr_t)on_wild);")}`,
      notes: ["An empty box slot makes box_ref fail, so the loop skips it. A box holds 30 slots."],
    },
    {
      slug: "no-battle-revives",
      title: "Block revive items in battle only",
      goal: "Revive items work on the party screen but not from the bag in battle.",
      tier: ["rules"],
      events: ["mon.revive"],
      functions: [],
      steps: [
        "`mon.revive` fires when a fainted Pokemon is about to come back. `event.source` tells what brought it: 0 is an item on the party screen, 1 is a revive item in battle, 2 a move or ability, 3 a Centre, 4 the HP-restore power, 5 any other script heal.",
        "The rule guards on source 1 and refuses. In battle a refused revive item has no effect and the bag keeps the item.",
        "Every other source goes on as usual.",
      ],
      rule: rule(`
language 1
rule no_battle_revives on mon.revive
    when event.source == 1
    do refuse
`),
      notes: ["What a refusal does depends on the source: an item on the party screen says it has no effect and stays in the bag, Sacred Ash is used up and that member stays fainted, and a party heal leaves that member fainted and heals the others."],
    },
    {
      slug: "no-party-heals",
      title: "No party heals",
      goal: "Pokemon Centres, scripts and the HP-restore power heal nobody.",
      tier: ["rules"],
      events: ["heal"],
      functions: [],
      steps: [
        "`heal` is the whole party heal. A rule that refuses it stops the heal: no member event fires and nobody is healed.",
        "A script's own text still plays.",
      ],
      rule: rule(`
language 1
rule no_party_heals on heal
    do refuse
`),
      notes: ["To spare one member and heal the rest, refuse `mon.revive` (a fainted member) or `heal.hp` (a hurt one) for that member."],
    },
    {
      slug: "no-battle-cures",
      title: "Block status cures in battle only",
      goal: "Status cures work on the party screen but not from the bag in battle.",
      tier: ["rules"],
      events: ["mon.cure"],
      functions: [],
      steps: [
        "`mon.cure` fires when a status is about to be cured. `event.source` is 0 for an item on the party screen, 1 in a battle and 2 for a party heal.",
        "The rule guards on source 1 and refuses. The status stays, and every other source goes on as usual.",
      ],
      rule: rule(`
language 1
rule no_battle_cures on mon.cure
    when event.source == 1
    do refuse
`),
      notes: ["A refused cure in a battle can still show the game's text that the status was cured. To stop the item from being used, refuse `battle.item`."],
    },
    {
      slug: "halve-pp-restores",
      title: "Halve PP restores",
      goal: "Every PP restore gives half the PP, on the field and in battle.",
      tier: ["rules", "c"],
      events: ["mon.pp"],
      functions: [],
      steps: [
        "`mon.pp` fires before PP is added. `event.amount` is the PP the move gets, and is writable.",
        "The amount is capped at the move's maximum after the rule runs. A refusal gives no PP.",
      ],
      rule: rule(`
language 1
rule half_pp on mon.pp
    do set event.amount event.amount / 2
`),
      c: `#include "eidos_wasm.h"

static int32_t on_pp(EidosEvent *event)
{
    EidosMonPp *e = (EidosMonPp *)event;
    e->amount /= 2;
    return EIDOS_EVENT_CONTINUE;
}

${start("    return eidos_on(EIDOS_EVENT_MON_PP, (uint32_t)(uintptr_t)on_pp);")}`,
    },
    {
      slug: "keep-one-hurt",
      title: "One Pokemon is not healed by a party heal",
      goal: "A party heal heals everyone except Pokemon of species 25.",
      tier: ["rules"],
      events: ["heal.hp", "mon.revive"],
      functions: [],
      steps: [
        "A party heal fires `heal.hp` once for each member that has lost HP, with source 2, and `mon.revive` once for each fainted member. A refusal affects that member only.",
        "The two rules are two files, one for each event. Together they leave the species hurt or fainted.",
      ],
      rule: rule(`
language 1
rule keep_hurt on heal.hp
    when event.source == 2 && species[event.mon] == 25
    do refuse
`),
      rule2: rule(`
language 1
rule keep_fainted on mon.revive
    when event.source >= 3 && species[event.mon] == 25
    do refuse
`),
      notes: ["The first block is the rule for `heal.hp`; the second is the rule for `mon.revive`. Revive sources 3, 4 and 5 are the party heals: a Centre, the HP-restore power and any other script."],
    },
    {
      slug: "no-battle-exp",
      title: "No experience from battles",
      goal: "Nobody gains experience from a battle.",
      tier: ["rules"],
      events: ["battle.exp"],
      functions: [],
      steps: [
        "`battle.exp` fires once when the battle hands out experience. Refused, nobody gains and `mon.exp` does not fire.",
        "Add a guard such as `event.wild == 1` to limit it to wild battles.",
      ],
      rule: rule(`
language 1
rule no_battle_exp on battle.exp
    do refuse
`),
    },
    {
      slug: "no-exp-for-species",
      title: "One Pokemon gains no experience",
      goal: "Pokemon of species 25 gain nothing from battles; the rest gain as usual.",
      tier: ["rules"],
      events: ["mon.exp"],
      functions: [],
      steps: [
        "`mon.exp` fires once for each member that gains. Setting `event.amount` to 0 gives that member none.",
        "`battle.exp` is the whole hand-out; use it to stop everyone, and `mon.exp` to pick members.",
      ],
      rule: rule(`
language 1
rule no_exp_pikachu on mon.exp
    when species[event.mon] == 25 && event.source == 0
    do set event.amount 0
`),
    },
    {
      slug: "no-revives",
      title: "Nothing comes back from fainting",
      goal: "A fainted Pokemon stays fainted: no revive item, move, Centre or script heal brings it back.",
      tier: ["rules"],
      events: ["mon.revive"],
      functions: [],
      steps: [
        "`mon.revive` fires for every way a fainted Pokemon comes back, and it can be refused. A rule with no guard refuses them all.",
        "At a party heal the event fires once for each fainted member, and a refusal leaves only that member fainted. The others are healed.",
      ],
      rule: rule(`
language 1
rule no_revives on mon.revive
    do refuse
`),
      notes: ["The Pokemon Centre's nurse runs the same script command as other script heals, so the port reports both as source 5 and a rule cannot tell them apart."],
    },
    {
      slug: "monotype",
      title: "Check types in the party",
      goal: "Refuse a ball on a species without a type, and refuse wild encounters while the party has no Pokemon of that type.",
      tier: ["rules"],
      events: ["catch.attempt", "encounter.wild"],
      functions: ["message"],
      steps: [
        "Type ids are the game's own: Water is 10.",
        "`species_type1[s]` and `species_type2[s]` read a species' types. The first rule refuses the ball when neither is Water.",
        "`party_has_type[t]` is 1 when a party Pokemon that is not an egg has type `t`. The second rule refuses the encounter when it is 0.",
        "A rule file holds one rule, so these are two files.",
      ],
      rule: rule(`
language 1
rule water_only on catch.attempt
    when species_type1[event.species] != 10 && species_type2[event.species] != 10
    do refuse
    do message "Only Water types."
`),
      rule2: rule(`
language 1
rule need_water on encounter.wild
    when party_has_type[10] == 0
    do refuse
`),
      notes: ["The first block is the rule for `catch.attempt`; the second is the rule for `encounter.wild`."],
    },
    {
      slug: "play-time",
      title: "Use the play time",
      goal: "Wild encounters start only after the save has an hour of play time.",
      tier: ["rules"],
      events: ["encounter.wild"],
      functions: ["play_seconds"],
      steps: ["`play_seconds` is the play time of the save in seconds.", "`encounter.wild` can be refused. The player keeps walking."],
      rule: rule(`
language 1
rule slow_start on encounter.wild
    when play_seconds < 3600
    do refuse
`),
    },
    {
      slug: "dark-grass-doubles",
      title: "Stronger second Pokemon in dark grass",
      goal: "In a double battle from dark grass, the second wild Pokemon is two levels higher.",
      tier: ["rules"],
      events: ["encounter.wild"],
      functions: [],
      steps: [
        "`event.method` is 1 for dark grass and `event.double` is 1 for a double battle.",
        "`event.level2` and `event.species2` are the second Pokemon of the double. Both are writable. `level2` is 0 when there is no second Pokemon.",
      ],
      rule: rule(`
language 1
rule dark_grass_pair on encounter.wild
    when event.method == 1 && event.double == 1
    do add event.level2 2
`),
    },
    {
      slug: "stronger-trainers",
      title: "Change a trainer's party",
      goal: "Every Pokemon of every trainer is a number of levels higher, and the player picks the number.",
      tier: ["rules"],
      events: ["battle.trainer_mon"],
      functions: ["setting_get"],
      steps: [
        "`battle.trainer_mon` fires once for each opposing Pokemon, in party order, after the game builds the party.",
        "A new level works out the stats again. Writing 0 to `level`, `species` or `ability` keeps the game's value.",
        "The option is a whole number. `setting.boost` reads it. It is declared in the options file with a lowest value, a highest value and a default.",
      ],
      settings: `option boost int "Extra trainer levels" 0 20 3\n`,
      rule: rule(`
language 1
rule stronger_trainers on battle.trainer_mon
    do add event.level setting.boost
`),
    },
    {
      slug: "no-grinding",
      title: "Change experience by source",
      goal: "Wild battles give no experience.",
      tier: ["rules"],
      events: ["mon.exp"],
      functions: [],
      steps: [
        "`event.source` is 0 for a battle and 1 for the day care.",
        "`event.wild` is 1 when the battle is a wild one.",
        "The event fires once for each member that gains experience, and an amount of 0 gives that member none.",
      ],
      rule: rule(`
language 1
rule no_grinding on mon.exp
    when event.source == 0 && event.wild == 1
    do set event.amount 0
`),
    },
    {
      slug: "graveyard-box",
      title: "Move a Pokemon to a box",
      goal: "A Pokemon of the player that faints is moved to box 23.",
      tier: ["rules"],
      events: ["battle.faint"],
      functions: ["mon_to_box_n"],
      steps: [
        "`box <mon> <n>` moves a party Pokemon to box `n`, counted from 0. It is applied at the next field step, so the battle is not disturbed.",
        "It fails when the box is full, and it never leaves the party without a Pokemon that is not an egg.",
      ],
      rule: rule(`
language 1
rule graveyard on battle.faint
    when event.side == 0
    do box event.mon 23
`),
    },
  ],
};
