<script lang="ts">
  import { tick } from "svelte";

  // A drop-down with a search box, for lists too long to scroll: species,
  // items, moves, trainers, maps. Type a name or a number to narrow it.
  let {
    value = $bindable(),
    options,
    disabled = false,
    label = "",
    icon,
    onchange,
  }: {
    value: number;
    /// [value, text] pairs, or plain names whose index is the value.
    options: [number, string][] | string[];
    disabled?: boolean;
    label?: string;
    /// A small picture per value, when the entries have one.
    icon?: (value: number) => string | null;
    onchange?: (value: number) => void;
  } = $props();

  const LIMIT = 200;

  let open = $state(false);
  let query = $state("");
  let active = $state(0);
  let root = $state<HTMLDivElement | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  let list = $state<HTMLUListElement | null>(null);
  let above = $state(false);

  const pairs = $derived.by((): [number, string][] =>
    options.length && typeof options[0] === "string"
      ? (options as string[]).map((n, i) => [i, n || `(${i})`])
      : (options as [number, string][]),
  );
  const current = $derived(pairs.find(([v]) => v === value)?.[1] ?? `(${value})`);
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return pairs.slice(0, LIMIT);
    const starts: [number, string][] = [];
    const has: [number, string][] = [];
    for (const p of pairs) {
      const t = p[1].toLowerCase();
      if (String(p[0]) === q || t.startsWith(q)) starts.push(p);
      else if (t.includes(q)) has.push(p);
      if (starts.length >= LIMIT) break;
    }
    return [...starts, ...has].slice(0, LIMIT);
  });

  async function show() {
    if (disabled) return;
    open = true;
    query = "";
    active = Math.max(0, shown.findIndex(([v]) => v === value));
    // Open upward when there is no room below.
    const r = root?.getBoundingClientRect();
    above = !!r && window.innerHeight - r.bottom < 280 && r.top > window.innerHeight - r.bottom;
    await tick();
    input?.focus();
    scrollActive();
  }

  function pick(v: number) {
    open = false;
    if (v !== value) {
      value = v;
      onchange?.(v);
    }
  }

  function scrollActive() {
    list?.children[active]?.scrollIntoView({ block: "nearest" });
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.max(0, Math.min(shown.length - 1, active + (e.key === "ArrowDown" ? 1 : -1)));
      scrollActive();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (shown[active]) pick(shown[active][0]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      open = false;
    }
  }

  function onFocusOut(e: FocusEvent) {
    if (!root?.contains(e.relatedTarget as Node | null)) open = false;
  }
</script>

<div class="search-select" bind:this={root} onfocusout={onFocusOut}>
  <button type="button" class="current" {disabled} onclick={() => (open ? (open = false) : show())} aria-haspopup="listbox" aria-expanded={open} aria-label={label}>
    {#if icon && icon(value)}<img class="px icon" src={icon(value)} alt="" />{/if}
    <span>{current}</span>
    <i class="caret"></i>
  </button>
  {#if open}
    <div class="pop window" class:above>
      <input
        bind:this={input}
        bind:value={query}
        oninput={() => (active = 0)}
        onkeydown={onKey}
        placeholder="Search"
        aria-label="Search"
      />
      <ul bind:this={list} role="listbox">
        {#each shown as [v, text], i (v)}
          <li>
            <button
              type="button"
              role="option"
              aria-selected={v === value}
              class:active={i === active}
              onmousedown={(e) => e.preventDefault()}
              onclick={() => pick(v)}
              onmouseenter={() => (active = i)}
            >
              {#if icon}
                {@const src = icon(v)}
                {#if src}<img class="px icon" {src} alt="" loading="lazy" />{:else}<span class="icon"></span>{/if}
              {/if}
              <span class="text">{text}</span>
            </button>
          </li>
        {:else}
          <li class="none">Nothing matches.</li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .search-select {
    position: relative;
    min-width: 0;
    width: 100%;
  }
  .current {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
    min-width: 0;
    text-align: left;
    font-weight: normal;
  }
  .current span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .caret {
    flex: none;
    width: 0;
    height: 0;
    border-top: 5px solid currentColor;
    border-left: 4px solid transparent;
    border-right: 4px solid transparent;
    opacity: 0.7;
  }
  .pop {
    position: absolute;
    z-index: 50;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    min-width: 12rem;
    padding: 0.35rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .pop.above {
    top: auto;
    bottom: calc(100% + 4px);
  }
  .pop input {
    width: 100%;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 16rem;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .icon {
    flex: none;
    width: 24px;
    height: 24px;
    object-fit: contain;
    image-rendering: pixelated;
  }
  .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  li button {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
    text-align: left;
    padding: 0.2rem 0.45rem;
    border: none;
    box-shadow: none;
    background: transparent;
    font-weight: normal;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  li button.active {
    background: color-mix(in srgb, var(--gold) 30%, transparent);
  }
  li button[aria-selected="true"] {
    font-weight: 700;
  }
  .none {
    padding: 0.3rem 0.45rem;
    color: var(--muted);
  }
</style>
