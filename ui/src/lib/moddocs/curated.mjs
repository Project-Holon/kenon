// kenon's own text for the mod docs: when each event fires, what each
// function does, the rules-language pages and the recipes. tools/mod-docs.mjs
// merges it with what it reads from eidos and writes api.json. Facts about
// the API come from eidos (docs/mod-*.md, sdk/*.h); only add text here that
// you can trace to one of those.

const rule = (s) => s.replace(/^\n/, "").replace(/\n\s*$/, "\n");

// Handler skeleton shared by the C recipes (native tier, api 3).
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
    heal: { when: "A heal starts, at a Pokemon Centre, from a script or from the HP-restore power. A handler may choose who heals." },
    "badge.get": { when: "A badge is set. It fires after the badge is held." },
    "flag.set": { when: "The game sets an event flag." },
    "dex.register": { when: "A species is registered as seen or caught." },
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
    setting_get: { what: "Reads one of the mod's own options (declared in the file named by the manifest's settings key).", result: "0 or 1 for a bool, the index for a choice.", tiers: ["c"] },
    state_load: { what: "Reads the mod's own saved data, kept per save slot.", result: "Bytes read, -1 on damage.", tiers: ["c"] },
    state_store: { what: "Writes the mod's own saved data (up to 4 MiB).", result: "0 on success.", tiers: ["c"] },
    state_clear: { what: "Deletes the mod's saved data.", tiers: ["c"] },
    state_info: { what: "Reads the header of the mod's saved data.", tiers: ["c"] },
    state_dir: { what: "The path of the mod's own folder.", tiers: ["c"] },
    state_in_step: { what: "Whether the mod's saved data was written together with the game save now in force.", result: "1 in step, 0 out of step, -1 unknown.", tiers: ["c"] },
  },

  functionGroups: [
    { title: "Entry points", fns: ["on","script"] },
    { title: "The party", fns: ["party_count","party_ref","party_get","party_set"] },
    { title: "The boxes", fns: ["box_count","box_ref","box_get","box_set"] },
    { title: "A Pokemon in battle", fns: ["battle_ref","battle_get","battle_set"] },
    { title: "Species and dex", fns: ["species_value","species_shiny","dex_caught"] },
    { title: "Badges, money and flags", fns: ["badge_count","badge_has","money","money_set","flag","flag_set"] },
    { title: "The map and the player", fns: ["zone","player_pos","battle_style"] },
    { title: "Moves between places", fns: ["mon_to_box","mon_take_item"] },
    { title: "Messages", fns: ["message","confirm"] },
    { title: "Peer sessions", fns: ["session_host","session_join","session_leave","session_send","on_session"] },
    { title: "Field avatars", fns: ["avatar_create","avatar_move","avatar_delete"] },
    { title: "Pokemon export and import", fns: ["mon_export","mon_import"] },
    { title: "Status panel, timer and splits", fns: ["panel_set","panel_clear","timer_start","timer_stop","timer_ms","split"] },
    { title: "Run, random, input and frame", fns: ["run_restart","rand_seed","rand_next","input_press","on_frame"] },
    { title: "External control and save handoff", fns: ["on_command","publish","save_send","on_save_received"] },
    { title: "Settings and saved data", fns: ["setting_get","state_load","state_store","state_clear","state_info","state_dir","state_in_step"] },
  ],

  actions: [
    { name: "refuse", syntax: "do refuse", what: "Refuses the event. Valid only on events that can be refused; the game then takes the event's refused outcome.", example: "do refuse" },
    { name: "set", syntax: "do set <target> <expr>", what: "Assigns a payload field marked writable, a writable state field, or a mod cell.", example: "do set event.amount 0" },
    { name: "add", syntax: "do add <target> <expr>", what: "Adds to a target.", example: "do add money 500" },
    { name: "sub", syntax: "do sub <target> <expr>", what: "Subtracts from a target.", example: "do sub money 1000" },
    { name: "message", syntax: "do message <text>", what: "Shows a line of text to the player.", example: "do message \"Be careful.\"" },
    { name: "send", syntax: "do send <expr>", what: "Sends a value to the peers of the current session.", example: "do send event.species" },
  ],

  pages: [
    {
      slug: "code-mods",
      section: "functions",
      title: "Code mods at a glance",
      blocks: [
        { t: "p", text: "A code mod is C. It exports eidos_mod_start and eidos_mod_stop, binds handlers to events with `on`, and reaches the game only through the state calls and services listed here. It cannot call game functions directly; raw hooks replace or wrap them." },
        { t: "p", text: "A handler gets the event payload and returns CONTINUE or REFUSE. It may write the payload fields marked writable; the port reads them back before the game goes on. A write to any other field has no effect. A refusal at an event that cannot be refused is ignored and logged once." },
        { t: "list", items: ["The snippets in these docs use the native header (mod_api.h and mod_state.h, api 3), which is what the port loads today.", "The WebAssembly build (api 4) keeps the same event, payload and service names. Its headers come with eidos's published api, and the build in the Code view uses the current draft header until then.", "No libc and no WASI in the wasm build: a mod reaches nothing outside the game except through the port's imports."] },
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
        { t: "p", text: "Operators are read as two characters at most, so write `a < -1` with spaces." },
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
      tier: ["c"],
      events: ["mon.exp"],
      functions: ["setting_get", "on"],
      steps: [
        "Declare the option in a settings file and name it in the manifest with `settings = settings.txt`.",
        "In the handler for `mon.exp`, read the option with `setting_get`. It returns 0 or 1 for a bool.",
        "`amount` is writable, so the handler halves it before the game uses it.",
        "Rules cannot read options yet; this needs code.",
      ],
      settings: `option mymod.slow_exp bool "Slow experience" off\n`,
      c: `#include "mod_api.h"
#include "mod_state.h"

static const BwModHost *g_host;

static int on_exp(void *mod, BwEvent *event)
{
    BwMonExp *e = (BwMonExp *)event;
    (void)mod;
    if (g_host->services->setting_get("mymod.slow_exp") == 1) {
        e->amount /= 2;
    }
    return BW_EVENT_CONTINUE;
}

${start("    return host->on(BW_EVENT_MON_EXP, on_exp);")}`,
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
  ],
};
