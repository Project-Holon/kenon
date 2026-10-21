<script lang="ts">
  import { onMount } from "svelte";
  import { archives, members, type ArchiveInfo, type MemberInfo } from "../lib/backend";
  import { play } from "../lib/sfx";
  import VirtualList from "../lib/VirtualList.svelte";

  let { version }: { version: number } = $props();

  let list = $state<ArchiveInfo[]>([]);
  let current = $state<number | null>(null);
  let items = $state<MemberInfo[]>([]);
  let filter = $state("");
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      list = await archives();
    } catch (e) {
      error = String(e);
    }
  });

  async function open(id: number) {
    play("select");
    current = id;
    items = await members(id);
  }

  $effect(() => {
    void version;
    if (current !== null) members(current).then((m) => (items = m));
  });

  const shown = $derived(list.filter((a) => a.path.includes(filter.trim())));
  const summary = (kinds: Record<string, number>) =>
    Object.entries(kinds)
      .sort((a, b) => b[1] - a[1])
      .map(([k, n]) => `${n} ${k}`)
      .join(", ");
  const size = (n: number) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(1)} KB`);
</script>

<div class="files">
  <aside class="archives">
    <input class="search" placeholder="Filter, such as a/0/2" bind:value={filter} />
    {#if error}<p class="bad">{error}</p>{/if}
    <VirtualList items={shown} height={34} key={(a) => a.id} label="Archives">
      {#snippet row(a)}
        <button class="row-item" aria-current={a.id === current} onclick={() => open(a.id)}>
          <span class="label"><code>{a.path}</code></span>
          <span class="tag">{a.members}</span>
        </button>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{list.length ? "Nothing matches." : "Reading the archives..."}</p>
      {/snippet}
    </VirtualList>
  </aside>
  <section class="members">
    {#if current === null}
      <p class="muted pad">Choose an archive.</p>
    {:else}
      <div class="head">
        <h2><code>{list.find((a) => a.id === current)?.path}</code></h2>
        <p class="muted">{summary(list.find((a) => a.id === current)?.kinds ?? {})}</p>
      </div>
      <div class="scroll grid">
        {#each items as m (m.index)}
          <div class="member window">
            <strong>#{m.index}</strong>
            <span>{m.kind}</span>
            <span class="muted">{size(m.size)}{m.compressed ? ", compressed" : ""}</span>
            {#if m.changed}<span class="tag changed">changed</span>{/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .files {
    display: grid;
    grid-template-columns: 14rem minmax(0, 1fr);
    height: 100%;
  }
  .archives {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.75rem;
    min-height: 0;
    border-right: 2px solid var(--outline);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .members {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .head {
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--outline);
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
    align-content: start;
    gap: 0.5rem;
    padding: 1rem;
  }
  .member {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding: 0.5rem 0.75rem;
    font-size: 0.9rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
