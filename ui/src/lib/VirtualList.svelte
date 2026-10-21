<script lang="ts" generics="T">
  import type { Snippet } from "svelte";

  // A list that only draws the rows in view (plus a few around them), so a
  // list of thousands scrolls and filters without stalling. Rows have one
  // fixed height.
  let {
    items,
    height,
    key,
    row,
    empty,
    label,
  }: {
    items: T[];
    height: number;
    key: (item: T) => string | number;
    row: Snippet<[T, number]>;
    empty?: Snippet;
    label?: string;
  } = $props();

  const OVERSCAN = 8;
  let viewport = $state<HTMLDivElement | null>(null);
  let top = $state(0);
  let size = $state(600);

  const first = $derived(Math.max(0, Math.floor(top / height) - OVERSCAN));
  const last = $derived(Math.min(items.length, Math.ceil((top + size) / height) + OVERSCAN));
  const visible = $derived(items.slice(first, last));

  // A new list (a search) starts at the top.
  $effect(() => {
    void items;
    if (viewport && viewport.scrollTop > items.length * height) viewport.scrollTop = 0;
  });

  $effect(() => {
    if (!viewport) return;
    const observer = new ResizeObserver(() => (size = viewport!.clientHeight));
    observer.observe(viewport);
    return () => observer.disconnect();
  });

  /// Brings row `index` into view.
  export function reveal(index: number) {
    if (!viewport) return;
    const y = index * height;
    if (y < viewport.scrollTop || y + height > viewport.scrollTop + viewport.clientHeight) {
      viewport.scrollTop = y - viewport.clientHeight / 2;
    }
  }
</script>

<div class="viewport" bind:this={viewport} onscroll={() => (top = viewport!.scrollTop)} role="listbox" aria-label={label}>
  {#if items.length === 0}
    {@render empty?.()}
  {:else}
    <div class="spacer" style="height: {items.length * height}px">
      <div class="rows" style="transform: translateY({first * height}px)">
        {#each visible as item, i (key(item))}
          <div class="row" style="height: {height}px">{@render row(item, first + i)}</div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .viewport {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .spacer {
    position: relative;
  }
  .rows {
    position: absolute;
    inset: 0 0 auto 0;
  }
  .row {
    display: flex;
    align-items: stretch;
    padding: 1px 0;
  }
  .row > :global(*) {
    flex: 1;
    min-width: 0;
  }
</style>
