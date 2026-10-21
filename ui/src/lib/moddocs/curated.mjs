// kenon's own text for the mod docs: when each event fires, what each
// function does, the rules-language pages and the recipes. tools/mod-docs.mjs
// merges it with what it reads from eidos and writes api.json. Facts about
// the API come from eidos (docs/mod-*.md, sdk/*.h); only add text here that
// you can trace to one of those.

const rule = (s) => s.replace(/^\n/, "").replace(/\n\s*$/, "\n");

// Handler skeleton shared by the C recipes (native tier, api 4).
const start = (reg) => `int eidos_mod_start(const BwModHost *host, void **mod_state)
{
    g_host = host;
    *mod_state = 0;
${reg}
}

void eidos_mod_stop(void *mod_state)
{
    (void)mod_state;
}
`;

export default {
  events: {
    "game.new": { when: "The player starts a new game, before the first map loads." },
    "save.begin": { when: "A save starts, from the save menu, a script or a link feature." },
    "save.done": { when: "A save step returns. The payload says whether the write was accepted." },
    "save.load": { when: "A save is loaded, when the game data is read from it." },
    "game.cleared": { when: "The ending begins: Hall of Fame, credits, then the save screen." },
    "route.change": { when: "The game sets up a new zone, on every map change." },
    "map.warp": { when: "A warp is about to move the player. A handler may change the destination." },
    "player.step": { when: "The game checks step events after the player moves one cell." },
    "trainer.spotted": { when: "A trainer looks for the player in line of sight. Refusing makes the trainer miss the player." },
    "npc.talk": { when: "The player presses A and the game looks for a person in front. Refusing finds nobody." },
    "menu.open": { when: "The X-button menu is about to open. Refusing keeps it shut." },
    "menu.build": { when: "The X-button menu has just been built. A handler may hide entries." },
    "field.move": { when: "The game asks whether a field move can be used. Refusing removes it from the choices." },
    "pc.open": { when: "A script opens the box screen (the PC, or the battle box)." },
    "encounter.wild": { when: "A wild Pokemon was rolled for the area the player walks in. A handler may change it or cancel the battle." },
    "encounter.static": { when: "A script starts a battle with a fixed Pokemon, such as a legendary or a gift battle." },
    "encounter.trainer": { when: "A trainer battle is being set up." },
    "mon.create": { when: "The game creates a Pokemon record, for any reason. A handler may change species, level, shininess and nature." },
    "mon.receive": { when: "The player gets a Pokemon: caught, gifted, an egg or an in-game trade. It fires after the Pokemon is stored." },
    "mon.nickname": { when: "The name prompt opens, or the catch screen offers a nickname." },
    "mon.evolve": { when: "The game checks whether a Pokemon evolves, after a level-up, with a stone or on a trade. Refusing stops the evolution." },
    "mon.exp": { when: "Experience from a battle or the Day Care is about to be added. A handler may change the amount." },
    "mon.levelup": { when: "A Pokemon in battle gains a level. It fires once for the level crossing." },
    "move.learn": { when: "A Pokemon is about to learn a move by level-up, the Day Care, a TM or tutor, the move relearner or a script." },
    "mon.release": { when: "The player releases a Pokemon from the PC box screen." },
    "box.deposit": { when: "The player puts a party Pokemon into a box on the PC screen." },
    "box.withdraw": { when: "The player takes a Pokemon out of a box into the party on the PC screen." },
    "party.swap": { when: "Two party members change places." },
    "mon.helditem": { when: "The party screen gives a Pokemon a held item." },
    "egg.hatch": { when: "A script starts an egg hatch." },
    "trade.ingame": { when: "A script starts an in-game trade. Refusing cancels it and the script goes on." },
    "battle.start": { when: "A battle is created, then again when its setup is built. A handler may change the format and the opponent levels." },
    "battle.action": { when: "The game checks a fight, switch or run choice. For a fight it fires several times a turn, once per move checked." },
    "battle.item": { when: "The player picks an item from the bag in battle. Refusing returns to the action menu." },
    "battle.switch": { when: "A Pokemon is sent out in battle, including after a faint." },
    "battle.escape": { when: "A run attempt is resolved. Refusing makes it fail and spends the turn." },
    "battle.move": { when: "A Pokemon uses a move." },
    "battle.damage": { when: "The game has computed a hit and not applied it. A handler may change the amount." },
    "battle.faint": {
      when: "A Pokemon faints in battle, from either side. Poison on the field never faints a Pokemon in this game (it stops at 1 HP).",
      facts: [
        "mon is a battle reference (place BW_MON_BATTLE); side 0 is the player's. cause, cause_move and cause_damage are empty or 0 when the faint came from weather, a hazard or a status.",
        "A whiteout has its own event, whiteout. It is not the same moment as a single faint.",
      ],
    },
    "catch.attempt": { when: "The player throws a ball. A handler may change the catch rate or block the ball." },
    "catch.result": { when: "The catch roll has finished. The payload says whether it caught and how many times the ball shook." },
    "battle.end": { when: "The battle is over and the game reports the result." },
    whiteout: { when: "The player's whole team has fainted. It fires for the battle part and for the field part." },
    "item.add": { when: "An item is added to the bag, from any source. A handler may change the count." },
    "item.use": { when: "The party screen checks whether an item can be used on a Pokemon. Refusing gives the game's 'no effect' text." },
    "shop.buy": { when: "The player picks an item and a quantity in a shop. A handler may change the price." },
    "money.change": { when: "Money is added or taken. A handler may change the amount, but only in the direction of the call." },
    "heal.hp": {
      category: "Pokemon",
      when: "The game is about to add HP to a Pokemon. On the field it fires when the party screen restores HP with an item. In a battle it fires for every HP restore, through BTL_SERVER_FLOW_ApplyHpRecover: a bag item, a berry, Leftovers, a move or an ability. A handler may change the amount, or refuse to add nothing.",
      facts: [
        "source is 0 (BW_HEAL_HP_FIELD) on the field and 1 (BW_HEAL_HP_BATTLE) in a battle. The amount is capped at max HP after the handler runs.",
        "On the field the item is used up even when the restore is refused: refuse item.use to keep it.",
      ],
      functions: ["party_get", "battle_get", "item_count", "message"],
    },
    heal: { when: "A heal starts, at a Pokemon Centre, from a script or from the HP-restore power. A handler may choose who heals." },
    "badge.get": { when: "A badge is set. It fires after the badge is held." },
    "flag.set": { when: "The game sets an event flag." },
    "dex.register": { when: "A species is registered as seen or caught." },
    "battle.trainer_mon": { when: "Once for each Pokemon of the opposing trainer, in party order, after the game builds the party and before the battle starts. A rule may change the level, species, item, moves, ability and form." },
  },

  functions: {
    on: { group: "Entry points", what: "Binds a handler to an event. Call it from eidos_mod_start.", result: "0 when bound.", tiers: ["c"] },
    script: { group: "Entry points", what: "Binds a handler to one script command of the field VM or the battle effect VM, by opcode.", tiers: ["c"] },

    party_count: { what: "How many Pokemon the party holds.", tiers: ["c"], rule: "party_count" },
    party_ref: { what: "A reference to the party Pokemon in a slot.", result: "0 on success.", tiers: ["c"] },
    party_get: { what: "Reads one field of a party Pokemon. Field ids are the BW_MON_FIELD_* constants.", result: "0 on success; the value goes into out.", tiers: ["c"], rule: "species[m], level[m], hp[m], max_hp[m], type1[m], type2[m], held_item[m]" },
    party_set: { what: "Writes one field of a party Pokemon. During a battle the write does not reach the battle and is lost when the battle writes its results back.", result: "0 on success.", tiers: ["c"], rule: "set hp[m] value" },
    box_count: { what: "How many boxes are usable.", tiers: ["c"] },
    box_ref: { what: "A reference to the Pokemon in a box slot.", result: "0 on success.", tiers: ["c"] },
    box_get: { what: "Reads one field of a boxed Pokemon.", result: "0 on success.", tiers: ["c"] },
    box_set: { what: "Writes one field of a boxed Pokemon.", result: "0 on success.", tiers: ["c"] },
    battle_ref: { what: "A reference to the battler on a side and slot.", result: "0 on success.", tiers: ["c"] },
    battle_get: { what: "Reads one field of a Pokemon in battle. Use it, not party_get, while a battle runs.", result: "0 on success.", tiers: ["c"] },
    battle_set: { what: "Writes one field of a Pokemon in battle.", result: "0 on success.", tiers: ["c"] },
    species_value: { what: "Reads a personal parameter of a species: types, stats, catch rate.", result: "0 on success.", tiers: ["c"], rule: "species_type1[s], species_type2[s], species_bst[s]" },
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
    setting_get: { what: "Reads one of the mod's own options (declared in the file named by the manifest's settings key).", result: "0 or 1 for a bool, the index for a choice, the number for an int option.", tiers: ["rules", "c"], rule: "setting.key" },
    state_load: { what: "Reads the mod's own saved data, kept per save slot.", result: "Bytes read, -1 on damage.", tiers: ["c"] },
    state_store: { what: "Writes the mod's own saved data (up to 4 MiB).", result: "0 on success.", tiers: ["c"] },
    state_clear: { what: "Deletes the mod's saved data.", tiers: ["c"] },
    state_info: { what: "Reads the header of the mod's saved data.", tiers: ["c"] },
    state_dir: { what: "The path of the mod's own folder.", tiers: ["c"] },
    state_in_step: { what: "Whether the mod's saved data was written together with the game save now in force.", result: "1 in step, 0 out of step, -1 unknown.", tiers: ["c"] },
  },

  // One plain line for each rule read the port added in version 4. The state
  // page shows it instead of the engine's table row.
  state: {
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
    place: "Where a reference points: the BW_MON_* place id (party, box or battle).",
    side: "In a battle, the side a reference belongs to: 0 is the player's.",
    species_stage: "The evolution stage of species s: 0 for the first stage of a line.",
    catch_rate: "The catch rate of species s.",
    gender_ratio: "The gender ratio byte of species s.",
    growth: "The growth rate id of species s.",
    egg_group1: "The first egg group of species s.",
    egg_group2: "The second egg group of species s.",
    area: "The area the player is in: the place name the game shows on entry. Every floor of a cave and every building of a town share it.",
    area_of: "The area of zone z.",
    wild_battle: "1 when the battle in progress is a wild one.",
    play_seconds: "The play time of the save, in seconds.",
    owned: "1 when the party or a box holds a Pokemon of species s that is not an egg.",
    party_max_level: "The highest level in the party, eggs left out.",
    party_alive: "How many party Pokemon have HP left, eggs left out.",
    party_has_type: "1 when a party Pokemon that is not an egg has type t.",
    item_pocket: "The bag pocket of item i.",
    move_type: "The type id of move v.",
    move_power: "The power of move v.",
    move_class: "The damage class of move v.",
  },

  functionGroups: [
    { title: "Entry points", fns: ["on","script"] },
    { title: "The party", fns: ["party_count","party_ref","party_get","party_set"] },
    { title: "The boxes", fns: ["box_count","box_ref","box_get","box_set"] },
    { title: "A Pokemon in battle", fns: ["battle_ref","battle_get","battle_set"] },
    { title: "Species and dex", fns: ["species_value","species_shiny","dex_caught"] },
    { title: "Badges, money and flags", fns: ["badge_count","badge_has","money","money_set","flag","flag_set"] },
    { title: "The map and the player", fns: ["zone"] },
    { title: "Moves between places", fns: ["mon_to_box","mon_to_box_n","mon_from_box","mon_take_item","mon_release"] },
    { title: "Recalculating a Pokemon", fns: ["mon_recalc"] },
    { title: "The world, the battle and play time", fns: ["area_of","battle_wild","play_seconds"] },
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
        { t: "p", text: "A code mod is C. It exports eidos_mod_start and eidos_mod_stop, binds handlers to events with `on`, and reaches the game only through the state calls and services listed here. It cannot call game functions directly; raw hooks replace or wrap them." },
        { t: "p", text: "A handler gets the event payload and returns CONTINUE or REFUSE. It may write the payload fields marked writable; the port reads them back before the game goes on. A write to any other field has no effect. A refusal at an event that cannot be refused is ignored and logged once." },
        { t: "list", items: ["The snippets in these docs use the native header (mod_api.h and mod_state.h, api 4), which is what the port loads today. A mod built for api 3 still loads.", "The WebAssembly build (api 4) keeps the same event, payload and service names. Its headers come with eidos's published api, and the build in the Code view uses the current draft header until then.", "No libc and no WASI in the wasm build: a mod reaches nothing outside the game except through the port's imports."] },
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
        { t: "p", text: "Read the mod's own options as `setting.<key>` and its data tables as `table.<name>[key]`." },
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
        "Declare the option in a settings file and name it in the manifest with `settings = settings.txt`.",
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
      c: `#include "mod_api.h"
#include "mod_state.h"

static const BwModHost *g_host;

static int on_exp(void *mod, BwEvent *event)
{
    BwMonExp *e = (BwMonExp *)event;
    (void)mod;
    if (g_host->services->setting_get("slow_exp") == 1) {
        e->amount /= 2;
    }
    return BW_EVENT_CONTINUE;
}

${start("    return host->on(BW_EVENT_MON_EXP, on_exp);")}`,
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
      c: `#include "mod_api.h"
#include "mod_state.h"

static const BwModHost *g_host;

static int on_faint(void *mod, BwEvent *event)
{
    BwBattleFaint *e = (BwBattleFaint *)event;
    (void)mod;
    if (e->side == 0) {
        g_host->state->mon_release(e->mon);
    }
    return BW_EVENT_CONTINUE;
}

${start("    return host->on(BW_EVENT_BATTLE_FAINT, on_faint);")}`,
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
      c: `#include "mod_api.h"
#include "mod_state.h"

static const BwModHost *g_host;
static int g_given;

static int on_route(void *mod, BwEvent *event)
{
    BwRouteChange *e = (BwRouteChange *)event;
    (void)mod;
    if (e->zone == 115 && !g_given && g_host->state->item_add(17, 5) == 0) {
        g_given = 1;
    }
    return BW_EVENT_CONTINUE;
}

${start("    return host->on(BW_EVENT_ROUTE_CHANGE, on_route);")}`,
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
      c: `#include "mod_api.h"
#include "mod_state.h"

static const BwModHost *g_host;
static u32 g_faints;

static int on_faint(void *mod, BwEvent *event)
{
    BwBattleFaint *e = (BwBattleFaint *)event;
    char line[24] = "Faints: ";
    char digits[11];
    u32 n = 0, v, i;
    (void)mod;
    if (e->side != 0) {
        return BW_EVENT_CONTINUE;
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
    g_host->services->panel_set(0, line);
    return BW_EVENT_CONTINUE;
}

${start("    return host->on(BW_EVENT_BATTLE_FAINT, on_faint);")}`,
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
      c: `#include "mod_api.h"
#include "mod_state.h"

static const BwModHost *g_host;

static int on_exp(void *mod, BwEvent *event)
{
    BwMonExp *e = (BwMonExp *)event;
    s32 cap = 100;
    (void)mod;
    g_host->services->table_get("caps", (s32)g_host->state->badge_count(), &cap);
    if ((s32)e->level >= cap) {
        e->amount = 0;
    }
    return BW_EVENT_CONTINUE;
}

${start("    return host->on(BW_EVENT_MON_EXP, on_exp);")}`,
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
      notes: ["To skip only the first stage of an evolution line, add `species_stage[event.species] == 0` to the guard."],
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
        "The option is a whole number. `setting.boost` reads it. It is declared in the settings file with a minimum, a maximum and a default.",
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
