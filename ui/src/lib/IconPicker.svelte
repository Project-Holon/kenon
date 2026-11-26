<script lang="ts">
  // A short list whose choices have game icons (types): a compact box showing
  // the current choice, and a popup with every choice as icon and name, so
  // the whole list is seen at once instead of scrolled.
  let {
    value = $bindable(),
    options,
    icon,
    disabled = false,
    label,
  }: {
    value: number;
    options: string[];
    icon: (v: number) => string | null;
    disabled?: boolean;
    label: string;
  } = $props();

  let open = $state(false);
  let root = $state<HTMLElement | null>(null);

  function choose(i: number) {
    value = i;
    open = false;
  }
  // Closes on a click elsewhere or Escape.
  function outside(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }
  function key(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      open = false;
      e.stopPropagation();
    }
  }
  const current = $derived(Number(value));
</script>

<svelte:window onpointerdown={outside} onkeydown={key} />

<span class="picker" bind:this={root}>
  <button type="button" class="box" {disabled} aria-haspopup="listbox" aria-expanded={open} aria-label={label} onclick={() => (open = !open)}>
    {#if icon(current)}<img class="px" src={icon(current)} alt="" />{/if}
    <span class="name">{options[current] ?? current}</span>
    <span class="caret" aria-hidden="true"></span>
  </button>
  {#if open}
    <div class="pop window" role="listbox" aria-label={label}>
      {#each options as n, i}
        {#if n}
          <button type="button" role="option" aria-selected={i === current} class:on={i === current} onclick={() => choose(i)}>
            {#if icon(i)}<img class="px" src={icon(i)} alt="" />{/if}
            <span>{n}</span>
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</span>

<style>
  .picker {
    position: relative;
    display: block;
  }
  .box {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    text-align: left;
  }
  .box .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .box img {
    height: 12px;
  }
  .caret {
    border: 4px solid transparent;
    border-top-color: currentColor;
    margin-top: 4px;
  }
  .pop {
    position: absolute;
    z-index: 30;
    top: calc(100% + 4px);
    left: 0;
    display: grid;
    grid-template-columns: repeat(3, max-content);
    gap: 0.25rem;
    padding: 0.5rem;
    background: var(--panel-2);
    border: 2px solid var(--outline);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.45);
  }
  .pop button {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0.55rem;
    background: none;
    border: 2px solid transparent;
    box-shadow: none;
    text-align: left;
    font-weight: 600;
  }
  .pop button:hover {
    background: var(--panel-3);
  }
  .pop button.on {
    border-color: var(--accent);
  }
  .pop img {
    height: 16px;
  }
  .px {
    image-rendering: pixelated;
  }
</style>
