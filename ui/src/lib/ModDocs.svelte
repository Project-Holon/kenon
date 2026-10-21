<script lang="ts">
  import { onMount, tick } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { copyText } from "./exporter";
  import {
    SECTIONS,
    buildEntries,
    cSkeleton,
    hookFamily,
    hookLine,
    loadApi,
    ruleSkeleton,
    type Api,
    type ApiEvent,
    type Block,
    type Entry,
    type Section,
  } from "./moddocs";
  import { buildIndex, queryMatcher, searchIndex, type SearchIndex } from "./moddocs/search";

  // A large window with the mod docs: events, functions, hookable game
  // functions, the rules language and worked recipes. `select` is the id of
  // the entry to show first, for example "event:battle.faint".
  let { onClose, select = null }: { onClose: () => void; select?: string | null } = $props();

  let dialog = $state<HTMLDialogElement | null>(null);
  let api = $state<Api | null>(null);
  let entries = $state<Entry[]>([]);
  let error = $state<string | null>(null);
  let query = $state("");
  let selected = $state<string | null>(null);
  const open = new SvelteSet<string>(["sec:events"]);
  let detail = $state<HTMLElement | null>(null);
  let index = $state.raw<SearchIndex | null>(null);
  let cursor = $state(0);

  // Find in the entry.
  let findOpen = $state(false);
  let findQuery = $state("");
  let findInput = $state<HTMLInputElement | null>(null);
  let findTotal = $state(0);
  let findAt = $state(0);
  // Set when the find text came from the sidebar search: stems match words.
  let smart: ((word: string) => boolean) | null = null;
  let spans: { node: Text; start: number; end: number }[] = [];
  let ranges: Range[] = [];
  let marks: HTMLElement[] = [];
  let lastEsc = 0;
  // The CSS highlight API paints matches without touching the text. Older
  // webviews get <mark> elements instead.
  const useHighlights = typeof CSS !== "undefined" && "highlights" in CSS && typeof Highlight !== "undefined";

  onMount(async () => {
    dialog?.showModal();
    try {
      api = await loadApi();
      entries = buildEntries(api);
      index = buildIndex(api, entries);
      go(select && entries.some((e) => e.id === select) ? select : "event:" + api.events[0].name);
    } catch (e) {
      error = String(e);
    }
  });

  const byId = $derived(new Map(entries.map((e) => [e.id, e])));
  const current = $derived(selected ? byId.get(selected) : undefined);
  const results = $derived(index && query.trim() ? searchIndex(index, query, 50) : []);

  function go(id: string, fromSearch = false) {
    const e = byId.get(id) ?? entries.find((x) => x.id === id);
    if (!e) return;
    selected = id;
    if (fromSearch && index) {
      findQuery = query.trim();
      smart = queryMatcher(index, query);
      findOpen = true;
    }
    open.add(`sec:${e.section}`);
    open.add(`grp:${e.section}:${e.group}`);
    tick().then(() => {
      detail?.scrollTo({ top: 0 });
      document.querySelector(".tree [aria-current='true']")?.scrollIntoView({ block: "nearest" });
    });
  }

  function toggle(key: string) {
    if (open.has(key)) open.delete(key);
    else open.add(key);
  }

  // The tree: sections, their groups in a fixed order, their entries.
  const tree = $derived.by(() => {
    if (!api) return [];
    const order: Record<Section, string[]> = {
      events: api.categories,
      functions: ["Code mods", ...api.functionGroups.map((g) => g.title)],
      hooks: ["Raw hooks"],
      rules: ["Language", "State fields"],
      recipes: ["Rules", "Code"],
    };
    return SECTIONS.map((s) => {
      const list = entries.filter((e) => e.section === s.id);
      const names = [...new Set(list.map((e) => e.group))];
      const known = order[s.id].filter((n) => names.includes(n));
      const rest = names.filter((n) => !known.includes(n)).sort();
      const groups = [...known, ...rest].map((n) => ({ name: n, items: list.filter((e) => e.group === n) }));
      return { ...s, groups };
    });
  });

  // `code` pieces of a text.
  const pieces = (text: string) => text.split(/(`[^`]+`)/).map((p) => (p.startsWith("`") ? { code: p.slice(1, -1) } : { text: p }));

  const evt = (id: string): ApiEvent | undefined => api?.events.find((e) => e.id === id);
  const hook = (id: string) => api?.hooks.find((h) => h.id === id);
  const fn = (id: string) => api?.functions.find((f) => f.id === id);
  const recipe = (id: string) => api?.recipes.find((r) => r.id === id);
  const stField = (id: string) => api?.state.find((s) => `state:${s.name}` === id);
  const page = (id: string) => (api ? [...api.pages, api.hookPage].find((p) => p.id === id) : undefined);
  const enumOf = (prefix: string) => api?.enums.find((g) => g.prefix === prefix);

  const eventHooks = (name: string) => api?.hooks.filter((h) => h.events.includes(name)) ?? [];
  const recipeTitle = (slug: string) => api?.recipes.find((r) => r.slug === slug)?.title ?? slug;
  const eventsOfFn = (name: string) => api?.functions.find((f) => f.name === name)?.events ?? [];
  const recipesOfFn = (name: string) => api?.recipes.filter((r) => r.functions.includes(name)) ?? [];
  const tierLabel = (t: string) => (t === "rules" ? "Rules" : t === "c" ? "C" : t);
  const unusedHooks = (h: { events: string[] }) => h.events.length === 0;

  function searchKey(e: KeyboardEvent) {
    if (!results.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      cursor = (cursor + (e.key === "ArrowDown" ? 1 : results.length - 1)) % results.length;
      tick().then(() => document.querySelector(".tree [data-active='true']")?.scrollIntoView({ block: "nearest" }));
    } else if (e.key === "Enter") {
      e.preventDefault();
      go(results[Math.min(cursor, results.length - 1)].id, true);
    }
  }

  // ---- find in the entry

  const WORD = /[A-Za-z0-9]+/g;

  function collect(): typeof spans {
    const q = findQuery.trim().toLowerCase();
    if (!detail || !q) return [];
    const nodes: Text[] = [];
    const walk = document.createTreeWalker(detail, NodeFilter.SHOW_TEXT);
    for (let n = walk.nextNode(); n; n = walk.nextNode()) if ((n as Text).data.trim()) nodes.push(n as Text);
    const out: typeof spans = [];
    const scan = (words: string[]) => {
      for (const node of nodes) {
        const low = node.data.toLowerCase();
        for (const w of words) {
          for (let at = low.indexOf(w); at >= 0; at = low.indexOf(w, at + w.length)) out.push({ node, start: at, end: at + w.length });
        }
      }
    };
    if (smart) {
      for (const node of nodes) {
        for (const m of node.data.matchAll(WORD)) if (smart(m[0])) out.push({ node, start: m.index, end: m.index + m[0].length });
      }
      return out;
    }
    scan([q]);
    const words = q.split(/\s+/).filter(Boolean);
    if (!out.length && words.length > 1) {
      scan(words);
      // Document order.
      const at = new Map(nodes.map((n, i) => [n, i]));
      out.sort((a, b) => at.get(a.node)! - at.get(b.node)! || a.start - b.start);
    }
    return out;
  }

  function clearFind() {
    if (useHighlights) {
      CSS.highlights.delete("docs-find");
      CSS.highlights.delete("docs-find-cur");
    }
    for (const m of marks) {
      const parent = m.parentNode;
      if (!parent) continue;
      parent.replaceChild(document.createTextNode(m.textContent ?? ""), m);
      parent.normalize();
    }
    marks = [];
    ranges = [];
    spans = [];
  }

  function runFind() {
    clearFind();
    if (!findOpen) return;
    spans = collect();
    findTotal = spans.length;
    if (useHighlights) {
      ranges = spans.map((s) => {
        const r = new Range();
        r.setStart(s.node, s.start);
        r.setEnd(s.node, s.end);
        return r;
      });
      if (ranges.length) CSS.highlights.set("docs-find", new Highlight(...ranges));
    } else {
      // Wrap from the end of each node so the offsets stay valid.
      const made: HTMLElement[] = [];
      for (const s of [...spans].reverse()) {
        s.node.splitText(s.end);
        const mid = s.node.splitText(s.start);
        const el = document.createElement("mark");
        el.className = "docs-find";
        el.textContent = mid.data;
        mid.parentNode?.replaceChild(el, mid);
        made.push(el);
      }
      marks = made.reverse();
    }
    mark(0, true);
  }

  function mark(i: number, scroll: boolean) {
    findAt = i;
    if (!findTotal) return;
    let el: Element | null | undefined;
    if (useHighlights) {
      CSS.highlights.set("docs-find-cur", new Highlight(ranges[i]));
      el = ranges[i].startContainer.parentElement;
    } else {
      marks.forEach((m, k) => m.classList.toggle("cur", k === i));
      el = marks[i];
    }
    if (scroll) el?.scrollIntoView({ block: "center" });
  }

  function step(d: number) {
    if (!findTotal) return;
    mark((findAt + d + findTotal) % findTotal, true);
  }

  async function openFind() {
    findOpen = true;
    await tick();
    findInput?.focus();
    findInput?.select();
  }

  function closeFind() {
    findOpen = false;
    clearFind();
    findTotal = 0;
    smart = null;
  }

  function findKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      step(e.shiftKey ? -1 : 1);
    }
  }

  // Re-run on a new entry at once, and while typing after a short pause.
  let lastShown: string | null = null;
  $effect(() => {
    const id = selected;
    void findQuery;
    void findOpen;
    const wait = id !== lastShown ? 0 : 120;
    lastShown = id;
    const t = setTimeout(runFind, wait);
    return () => clearTimeout(t);
  });

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      openFind();
      return;
    }
    // The first Esc closes the find bar, the next one the dialog.
    if (e.key === "Escape" && findOpen) {
      e.preventDefault();
      lastEsc = performance.now();
      closeFind();
      return;
    }
    if (e.key === "/" && document.activeElement?.tagName !== "INPUT") {
      e.preventDefault();
      document.getElementById("moddocs-search")?.focus();
    }
  }
</script>

{#snippet blk(b: Block)}
  {#if b.t === "p"}
    <p>
      {#each pieces(b.text) as p}{#if p.code !== undefined}<code>{p.code}</code>{:else}{p.text}{/if}{/each}
    </p>
  {:else if b.t === "code"}
    {@render snip(b.lang === "rule" ? "Rule" : b.lang === "hooks" ? "Hook line" : b.lang === "manifest" ? "Manifest line" : b.lang === "table" ? "Table file" : b.lang === "options" ? "Options file" : "Code", b.text)}
  {:else if b.t === "list"}
    <ul>
      {#each b.items as item}
        <li>{#each pieces(item) as p}{#if p.code !== undefined}<code>{p.code}</code>{:else}{p.text}{/if}{/each}</li>
      {/each}
    </ul>
  {:else}
    <table>
      <thead><tr>{#each b.head as h}<th>{h}</th>{/each}</tr></thead>
      <tbody>
        {#each b.rows as r}
          <tr>{#each r as c}<td>{#each pieces(c) as p}{#if p.code !== undefined}<code>{p.code}</code>{:else}{p.text}{/if}{/each}</td>{/each}</tr>
        {/each}
      </tbody>
    </table>
  {/if}
{/snippet}

{#snippet snip(label: string, text: string)}
  <div class="snippet">
    <div class="snip-head">
      <span class="muted">{label}</span>
      <button class="quiet" onclick={() => copyText(text)}>Copy</button>
    </div>
    <pre>{text}</pre>
  </div>
{/snippet}

{#snippet chev(isOpen: boolean)}
  <svg class="chev" class:open={isOpen} viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M6 3l5 5-5 5" /></svg>
{/snippet}

{#snippet link(id: string, label: string)}
  <button class="link" onclick={() => go(id)}>{label}</button>
{/snippet}

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialog}
  class="docs window"
  onclose={onClose}
  onclick={(e) => {
    if (e.target === dialog) dialog?.close();
  }}
  aria-label="Mod docs"
  oncancel={(e) => {
    if (performance.now() - lastEsc < 150) e.preventDefault();
  }}
>
  <header>
    <h2>Mod docs</h2>
    {#if api?.meta.eidosRevision}<span class="muted">API {api.meta.apiVersion}, eidos {api.meta.eidosRevision}</span>{/if}
    <div class="spacer"></div>
    <button class="icon" onclick={() => (findOpen ? closeFind() : openFind())} title="Find in this entry (Ctrl+F)" aria-label="Find in this entry" aria-pressed={findOpen}>
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><circle cx="6.5" cy="6.5" r="4.5" /><path d="M10 10l4 4" /></svg>
    </button>
    <button onclick={() => dialog?.close()}>Close</button>
  </header>

  {#if error}
    <p class="bad pad">{error}</p>
  {:else if !api}
    <p class="muted pad">Loading.</p>
  {:else}
    <div class="cols">
      <nav class="tree" aria-label="Docs index">
        <input id="moddocs-search" type="search" placeholder="Search names, fields, text" aria-label="Search the docs" bind:value={query} oninput={() => (cursor = 0)} onkeydown={searchKey} />
        <div class="scroll">
          {#if query.trim()}
            {#each results as r, i (r.id)}
              <button class="row-item res" data-active={i === cursor} aria-current={r.id === selected} onclick={() => go(r.id, true)} title={r.title}>
                <span class="line">
                  <span class="label">{#each r.titleParts as p}{#if p.hit}<b class="hit">{p.text}</b>{:else}{p.text}{/if}{/each}</span>
                  <span class="tag">{r.group}</span>
                </span>
                {#if r.snippet}
                  <span class="why"><span class="fld">{r.field}</span> {#each r.snippet as p}{#if p.hit}<b class="hit">{p.text}</b>{:else}{p.text}{/if}{/each}</span>
                {/if}
              </button>
            {:else}
              <p class="muted pad">Nothing found.</p>
            {/each}
          {:else}
            {#each tree as s (s.id)}
              <button class="sec" aria-expanded={open.has(`sec:${s.id}`)} onclick={() => toggle(`sec:${s.id}`)}>
                {@render chev(open.has(`sec:${s.id}`))}{s.title}
              </button>
              {#if open.has(`sec:${s.id}`)}
                {#each s.groups as g (g.name)}
                  <button class="grp" aria-expanded={open.has(`grp:${s.id}:${g.name}`)} onclick={() => toggle(`grp:${s.id}:${g.name}`)}>
                    {@render chev(open.has(`grp:${s.id}:${g.name}`))}{g.name}
                    <span class="count">{g.items.length}</span>
                  </button>
                  {#if open.has(`grp:${s.id}:${g.name}`)}
                    {#each g.items as it (it.id)}
                      <button class="row-item leaf" aria-current={it.id === selected} onclick={() => go(it.id)} title={it.title}>
                        <span class="label">{it.title}</span>
                      </button>
                    {/each}
                  {/if}
                {/each}
              {/if}
            {/each}
          {/if}
        </div>
      </nav>

      <div class="right">
      {#if findOpen}
        <div class="find" role="search">
          <input
            bind:this={findInput}
            type="search"
            placeholder="Find in this entry"
            aria-label="Find in this entry"
            bind:value={findQuery}
            oninput={() => (smart = null)}
            onkeydown={findKey}
          />
          <span class="muted count-find">{findQuery.trim() ? (findTotal ? `${findAt + 1} of ${findTotal}` : "No matches") : ""}</span>
          <button class="icon" onclick={() => step(-1)} disabled={!findTotal} title="Previous (Shift+Enter)" aria-label="Previous match">
            <svg class="chev up" viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M6 3l5 5-5 5" /></svg>
          </button>
          <button class="icon" onclick={() => step(1)} disabled={!findTotal} title="Next (Enter)" aria-label="Next match">
            <svg class="chev down" viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M6 3l5 5-5 5" /></svg>
          </button>
          <button class="icon" onclick={closeFind} title="Close (Esc)" aria-label="Close find">
            <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M3.5 3.5l9 9M12.5 3.5l-9 9" /></svg>
          </button>
        </div>
      {/if}
      <article class="detail" bind:this={detail}>
        {#if !current}
          <p class="muted">Choose an entry.</p>
        {:else if current.section === "events" && evt(current.id)}
          {@const e = evt(current.id)!}
          <h1><code>{e.name}</code></h1>
          <div class="tags">
            <span class="tag">{e.category}</span>
            <span class="tag">{e.refusable ? "Can be refused" : "Cannot be refused"}</span>
            {#if e.payload.some((f) => f.writable)}<span class="tag">Fields can be changed</span>{/if}
            {#if !e.ruleEvent}<span class="tag">Code only</span>{/if}
          </div>
          <p class="lead">{e.when}</p>

          <h3>Game function</h3>
          <p><code>{e.binding}</code> in {e.region}</p>
          {#if e.refuse}
            <h3>When refused</h3>
            <p>{e.refuse}</p>
          {/if}

          <h3>Payload</h3>
          {#if e.payload.length}
            <table class="fields">
              <thead><tr><th>Field</th><th>Type</th><th>Meaning</th></tr></thead>
              <tbody>
                {#each e.payload as f}
                  <tr>
                    <td><code>{f.name}</code>{#if f.writable}<span class="tag gold">writable</span>{/if}</td>
                    <td>{f.type}</td>
                    <td>{f.meaning}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
            <p class="muted small">
              In a rule, read a field as <code>event.{e.payload[0].name}</code>; only writable fields can be set. In C,
              the payload is <code>{e.cStruct}</code>.
            </p>
            {#each [...new Set(e.payload.map((f) => f.enum).filter((x) => !!x))] as pre}
              {@const g = enumOf(pre!)}
              {#if g}
                <p class="small"><span class="muted">{g.title}:</span> {#each g.members as m, i}<code>{m.value}</code> {m.name.replace(pre!, "").toLowerCase().replace(/_/g, " ")}{i < g.members.length - 1 ? ", " : ""}{/each}</p>
              {/if}
            {/each}
          {:else}
            <p class="muted">No fields: the event is the moment itself.</p>
          {/if}

          {#if e.notes.length || e.caveats.length}
            <h3>Details</h3>
            <ul class="small">
              {#each [...e.notes, ...e.caveats] as n}<li>{n}</li>{/each}
            </ul>
          {/if}

          {#if e.functions.length}
            <h3>Useful with it</h3>
            <p class="links">{#each e.functions as f}{@render link(`fn:${f}`, f)}{/each}</p>
          {/if}
          {#if e.recipes.length}
            <h3>Recipes</h3>
            <p class="links">{#each e.recipes as r}{@render link(`recipe:${r}`, recipeTitle(r))}{/each}</p>
          {/if}
          {#if eventHooks(e.name).length}
            <h3>Fires from</h3>
            <p class="links">{#each eventHooks(e.name) as h}{@render link(h.id, h.name)}{/each}</p>
          {/if}

          {#if e.ruleEvent}{@render snip("Rule skeleton", ruleSkeleton(e))}{/if}
          {@render snip("C skeleton", cSkeleton(e))}
        {:else if current.section === "functions" && fn(current.id)}
          {@const f = fn(current.id)!}
          <h1><code>{f.name}</code></h1>
          <div class="tags">
            <span class="tag">{f.group}</span>
            {#each f.tiers as t}<span class="tag">{tierLabel(t)}</span>{/each}
          </div>
          <p class="lead">{f.what}</p>
          {@render snip("Signature", f.signature + ";")}
          {#if f.params.length}
            <h3>Parameters</h3>
            <ul class="small">{#each f.params as p}<li><code>{p}</code></li>{/each}</ul>
          {/if}
          {#if f.result}
            <h3>Returns</h3>
            <p>{f.result}</p>
          {/if}
          {#if f.kind === "state"}
            <p class="muted small">
              Reach it through <code>host-&gt;state</code>. Every call returns 0 on success unless it says otherwise. The
              game's later write wins, and the battle works on a copy of each Pokemon.
            </p>
          {:else if f.kind === "service"}
            <p class="muted small">Reach it through <code>host-&gt;services</code>. The table has one copy per mod.</p>
          {/if}
          {#if f.rule}
            <h3>In a rule</h3>
            <p><code>{f.rule}</code></p>
          {/if}
          {#if f.events.length}
            <h3>Events it goes with</h3>
            <p class="links">{#each f.events as ev}{@render link(`event:${ev}`, ev)}{/each}</p>
          {/if}
          {#if recipesOfFn(f.name).length}
            <h3>Recipes</h3>
            <p class="links">{#each recipesOfFn(f.name) as r}{@render link(r.id, r.title)}{/each}</p>
          {/if}
        {:else if current.section === "hooks" && hook(current.id)}
          {@const h = hook(current.id)!}
          <h1><code>{h.name}</code></h1>
          <div class="tags">
            <span class="tag">{h.region}</span>
            <span class="tag">{hookFamily(api, h.name)}</span>
            {#if unusedHooks(h)}<span class="tag">No event</span>{/if}
          </div>
          <table class="fields">
            <tbody>
              <tr><td>Key</td><td><code>{h.key}</code></td></tr>
              {#if h.corpusName}<tr><td>Newer name</td><td><code>{h.corpusName}</code></td></tr>{/if}
              <tr><td>Prototype</td><td>{#if h.proto}<code>{h.proto}</code>{:else}<span class="muted">Not published for this function.</span>{/if}</td></tr>
            </tbody>
          </table>
          <p class="muted small">
            The group comes from the name's prefix. The docs give no description for most of these functions.
          </p>
          {#if h.events.length}
            <h3>Fires</h3>
            <p class="links">{#each h.events as ev}{@render link(`event:${ev}`, ev)}{/each}</p>
          {/if}
          {@render snip("Hook line", hookLine(h))}
          <p class="links">{@render link("page:hooks", "How raw hooks work")}</p>
        {:else if current.section === "recipes" && recipe(current.id)}
          {@const r = recipe(current.id)!}
          <h1>{r.title}</h1>
          <div class="tags">
            <span class="tag">Needs: {r.tier.map(tierLabel).join(" or ")}</span>
          </div>
          <p class="lead">{r.goal}</p>
          <h3>How it works</h3>
          <ol>
            {#each r.steps as s}<li>{#each pieces(s) as p}{#if p.code !== undefined}<code>{p.code}</code>{:else}{p.text}{/if}{/each}</li>{/each}
          </ol>
          {#if r.settings}{@render snip("settings.txt", r.settings)}{/if}
          {#each r.tables ?? [] as t}{@render snip(t.file, t.text)}{/each}
          {#if r.rule}{@render snip(r.rule2 ? "Rule 1" : "Rule", r.rule)}{/if}
          {#if r.rule2}{@render snip("Rule 2", r.rule2)}{/if}
          {#if r.c}
            {@render snip("C (native header, api 3)", r.c)}
          {/if}
          {#if r.notes?.length}
            <h3>Notes</h3>
            <ul class="small">{#each r.notes as n}<li>{#each pieces(n) as p}{#if p.code !== undefined}<code>{p.code}</code>{:else}{p.text}{/if}{/each}</li>{/each}</ul>
          {/if}
          <h3>Uses</h3>
          <p class="links">
            {#each r.events as ev}{@render link(`event:${ev}`, ev)}{/each}
            {#each r.functions as f}{@render link(`fn:${f}`, f)}{/each}
          </p>
        {:else if current.id === "page:actions"}
          <h1>Actions</h1>
          <p class="lead">What a rule can do after <code>do</code>.</p>
          <table class="fields">
            <thead><tr><th>Action</th><th>Meaning</th></tr></thead>
            <tbody>
              {#each api.actions as a}<tr><td><code>{a.syntax}</code></td><td>{a.what}</td></tr>{/each}
            </tbody>
          </table>
          {#each api.actions as a}{@render snip(a.name, a.example + "\n")}{/each}
        {:else if stField(current.id)}
          {@const s = stField(current.id)!}
          <h1><code>{s.name}{s.sub === "plain" ? "" : s.sub === "mon" ? "[mon]" : "[n]"}</code></h1>
          <div class="tags">
            <span class="tag">{s.readable ? "Readable" : "Not readable yet"}</span>
            <span class="tag">{s.writable ? (s.writableOk ? "Writable" : "Write not working yet") : "Read only"}</span>
          </div>
          <p class="lead">{s.value}</p>
          {#if s.subscript}<p>The subscript is {s.subscript}.</p>{/if}
          {#if s.note}<p class="muted">{s.note}</p>{/if}
          {@render snip("Rule", `when ${s.name}${s.sub === "plain" ? "" : s.sub === "mon" ? "[event.mon]" : "[0]"} > 0\n`)}
        {:else if page(current.id)}
          {@const p = page(current.id)!}
          <h1>{p.title}</h1>
          {#each p.blocks as b}{@render blk(b)}{/each}
        {/if}
      </article>
      </div>
    </div>
  {/if}
</dialog>

<style>
  .docs {
    width: min(76rem, calc(100vw - 2rem));
    height: min(52rem, calc(100vh - 3rem));
    padding: 0;
    flex-direction: column;
    color: inherit;
    overflow: hidden;
  }
  .docs[open] {
    display: flex;
  }
  .docs::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.6rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  header button {
    padding: 0.2rem 0.7rem;
  }
  .spacer {
    flex: 1;
  }
  .pad {
    padding: 1rem;
  }
  .cols {
    display: grid;
    grid-template-columns: 18rem minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }
  .tree {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.6rem;
    border-right: 2px solid var(--outline);
    min-height: 0;
    min-width: 0;
  }
  .scroll {
    overflow-y: auto;
    overflow-x: hidden;
    min-height: 0;
  }
  .sec,
  .grp {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
    text-align: left;
    background: transparent;
    border: 2px solid transparent;
    box-shadow: none;
    padding: 0.25rem 0.4rem;
  }
  .sec {
    font-weight: 700;
    margin-top: 0.3rem;
  }
  .grp {
    font-weight: 600;
    padding-left: 1.1rem;
    color: var(--muted);
  }
  .grp:hover,
  .sec:hover {
    background: var(--panel-2);
  }
  .chev {
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
    color: var(--faint);
    transition: transform 0.12s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .chev.up {
    transform: rotate(-90deg);
  }
  .chev.down {
    transform: rotate(90deg);
  }
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.25rem 0.45rem;
  }
  .icon svg:not(.chev) {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
  .icon .chev {
    color: inherit;
  }
  .right {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .find {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.8rem;
    border-bottom: 2px solid var(--outline);
    background: var(--panel-2);
  }
  .find input {
    flex: 1;
    min-width: 0;
  }
  .count-find {
    font-size: 0.85rem;
    min-width: 4.5rem;
    text-align: right;
    white-space: nowrap;
  }
  .detail {
    flex: 1;
  }
  .res {
    flex-direction: column;
    align-items: stretch;
    gap: 0.1rem;
  }
  .res[data-active="true"]:not([aria-current="true"]) {
    background: var(--panel-3);
  }
  .line {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    min-width: 0;
  }
  .why {
    font-size: 0.78rem;
    font-weight: 400;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .res[aria-current="true"] .why {
    color: inherit;
  }
  .fld {
    color: var(--faint);
    margin-right: 0.2rem;
  }
  .hit {
    color: var(--gold);
    font-weight: 700;
  }
  .res[aria-current="true"] .hit {
    color: inherit;
    text-decoration: underline;
  }
  :global(::highlight(docs-find)) {
    background: color-mix(in srgb, var(--gold) 45%, transparent);
    color: inherit;
  }
  :global(::highlight(docs-find-cur)) {
    background: var(--gold);
    color: #201a00;
  }
  :global(mark.docs-find) {
    background: color-mix(in srgb, var(--gold) 45%, transparent);
    color: inherit;
  }
  :global(mark.docs-find.cur) {
    background: var(--gold);
    color: #201a00;
  }
  .count {
    margin-left: auto;
    font-size: 0.75rem;
    color: var(--faint);
  }
  .leaf {
    padding-left: 2.4rem;
    font-family: ui-monospace, Consolas, monospace;
    font-size: 0.85rem;
  }
  .detail {
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0.9rem 1.2rem 1.5rem;
    min-width: 0;
    min-height: 0;
  }
  .detail h1 {
    overflow-wrap: anywhere;
  }
  .detail h3 {
    margin: 1.1rem 0 0.35rem;
  }
  .detail p,
  .detail li {
    overflow-wrap: anywhere;
  }
  .detail p {
    margin: 0.4rem 0;
  }
  .detail ul,
  .detail ol {
    margin: 0.3rem 0;
    padding-left: 1.3rem;
  }
  .lead {
    font-size: 1.02rem;
    margin: 0.6rem 0;
  }
  .small {
    font-size: 0.88rem;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-top: 0.4rem;
  }
  .tag {
    flex: none;
  }
  .tag.gold {
    background: var(--gold);
    color: #201a00;
    margin-left: 0.4rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
    margin: 0.4rem 0;
  }
  th,
  td {
    text-align: left;
    vertical-align: top;
    padding: 0.3rem 0.5rem;
    border-bottom: 1px solid var(--rim);
    overflow-wrap: anywhere;
  }
  th {
    color: var(--muted);
    font-weight: 600;
  }
  .fields td:first-child {
    width: 28%;
  }
  .fields td:nth-child(2) {
    width: 16%;
  }
  .snippet {
    margin: 0.8rem 0;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    background: var(--bg);
  }
  .snip-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.1rem 0.5rem;
    border-bottom: 1px solid var(--rim);
  }
  .snip-head button {
    padding: 0.1rem 0.5rem;
  }
  pre {
    margin: 0;
    padding: 0.5rem 0.7rem;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font: 0.85rem/1.45 ui-monospace, Consolas, monospace;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem 0.5rem;
  }
  .link {
    padding: 0.1rem 0.5rem;
    font-family: ui-monospace, Consolas, monospace;
    font-size: 0.85rem;
    font-weight: 500;
  }
</style>
