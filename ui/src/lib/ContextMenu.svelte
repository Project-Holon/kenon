<script lang="ts">
  import { tick } from "svelte";
  import { closeMenu, menu, toast, toasts } from "./menu.svelte";

  let layer = $state<HTMLDivElement | null>(null);
  let el = $state<HTMLDivElement | null>(null);
  let active = $state(-1);
  let left = $state(0);
  let top = $state(0);
  let returnTo: HTMLElement | null = null;

  const items = $derived(menu.items);
  const usable = (i: number) => {
    const item = items[i];
    return item !== undefined && item !== "-" && !item.disabled;
  };

  // The layer sits in the open modal dialog when there is one, else on the
  // page; a dialog that closes must not take it along.
  $effect(() => {
    if (!layer) return;
    void menu.open;
    void toasts.length;
    const dialog = document.querySelector<HTMLElement>("dialog[open]");
    const host = (menu.open ? menu.host : null) ?? dialog ?? document.body;
    if (layer.parentNode !== host) host.appendChild(layer);
  });

  // Placed at the pointer, then moved back inside the window.
  $effect(() => {
    if (!menu.open) return;
    const { x, y } = menu;
    left = x;
    top = y;
    active = -1;
    returnTo = document.activeElement as HTMLElement | null;
    tick().then(() => {
      if (!el) return;
      const r = el.getBoundingClientRect();
      left = Math.max(4, Math.min(x, window.innerWidth - r.width - 4));
      top = Math.max(4, Math.min(y, window.innerHeight - r.height - 4));
      el.focus({ preventScroll: true });
    });
  });

  function close(restore = true) {
    if (!menu.open) return;
    closeMenu();
    if (restore) returnTo?.focus?.({ preventScroll: true });
  }

  async function run(i: number) {
    const item = items[i];
    if (!usable(i) || item === "-") return;
    close();
    try {
      await item.run();
    } catch (e) {
      toast(String(e), true);
    }
  }

  function move(d: number) {
    const n = items.length;
    let i = active;
    for (let k = 0; k < n; k++) {
      i = (i + d + n) % n;
      if (usable(i)) {
        active = i;
        return;
      }
    }
  }

  function key(e: KeyboardEvent) {
    if (!menu.open) return;
    const handled = ["ArrowDown", "ArrowUp", "Home", "End", "Enter", " ", "Escape", "Tab"];
    if (!handled.includes(e.key)) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "ArrowDown") move(1);
    else if (e.key === "ArrowUp") move(-1);
    else if (e.key === "Home") {
      active = -1;
      move(1);
    } else if (e.key === "End") {
      active = 0;
      move(-1);
    } else if (e.key === "Enter" || e.key === " ") run(active);
    else close();
  }

  function outside(e: Event) {
    if (menu.open && el && !el.contains(e.target as Node)) close(false);
  }

  function scrolled(e: Event) {
    if (menu.open && !(e.target instanceof Node && el?.contains(e.target))) close(false);
  }
</script>

<svelte:window
  onkeydowncapture={key}
  onpointerdowncapture={outside}
  onscrollcapture={scrolled}
  onwheelcapture={scrolled}
  onresize={() => close(false)}
  onblur={() => close(false)}
/>

<div class="layer" bind:this={layer}>
{#if menu.open}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="menu window"
    role="menu"
    tabindex="-1"
    bind:this={el}
    style="left: {left}px; top: {top}px"
    oncontextmenu={(e) => e.preventDefault()}
  >
    {#each items as item, i}
      {#if item === "-"}
        <div class="sep" role="separator"></div>
      {:else}
        <button
          class="item"
          class:on={active === i}
          role="menuitem"
          disabled={item.disabled}
          tabindex="-1"
          onpointerenter={() => usable(i) && (active = i)}
          onclick={() => run(i)}
        >
          <span class="text">{item.label}</span>
          {#if item.hint}<span class="hint">{item.hint}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

{#if toasts.length}
  <div class="toasts" role="status" aria-live="polite">
    {#each toasts as t (t.id)}
      <div class="toast window" class:bad={t.bad}>{t.text}</div>
    {/each}
  </div>
{/if}
</div>

<style>
  .layer {
    display: contents;
  }
  .menu {
    position: fixed;
    z-index: 1000;
    min-width: 12rem;
    max-width: min(24rem, calc(100vw - 8px));
    max-height: calc(100vh - 8px);
    overflow-y: auto;
    padding: 0.3rem;
    border-radius: var(--radius-s);
    box-shadow:
      inset 0 0 0 1px var(--rim),
      0 6px 20px rgba(0, 0, 0, 0.45);
    outline: none;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    width: 100%;
    padding: 0.3rem 0.6rem;
    border: 2px solid transparent;
    border-radius: 4px;
    background: transparent;
    box-shadow: none;
    color: var(--ink);
    font-weight: 500;
    text-align: left;
  }
  .item.on {
    background: linear-gradient(90deg, var(--accent), var(--accent-dark));
    border-color: var(--outline);
    color: var(--accent-ink);
  }
  .item:disabled {
    color: var(--faint);
    cursor: default;
  }
  .text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    font-size: 0.8rem;
    color: var(--muted);
  }
  .item.on .hint {
    color: inherit;
    opacity: 0.8;
  }
  .sep {
    height: 2px;
    margin: 0.25rem 0.3rem;
    background: var(--rim);
    border-radius: 1px;
  }
  .toasts {
    position: fixed;
    z-index: 1100;
    left: 50%;
    bottom: 1rem;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    align-items: center;
    max-width: calc(100vw - 2rem);
    pointer-events: none;
  }
  .toast {
    padding: 0.4rem 0.9rem;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .toast.bad {
    color: var(--bad);
  }
</style>
