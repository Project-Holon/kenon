<script lang="ts">
  import { onMount } from "svelte";
  import { archives, members, type ArchiveInfo, type MemberInfo } from "../lib/backend";
  import { archiveGroup, archiveLabel, archiveNote, kindName } from "../lib/archiveLabels";
  import { ALL } from "../lib/categories";
  import { play } from "../lib/sfx";
  import { debounced } from "../lib/cache.svelte";
  import VirtualList from "../lib/VirtualList.svelte";

  // Every archive of the game, with what each holds. The lists in the
  // sidebar cover the common ones; this reaches the rest.
  let { version, onGo }: { version: number; onGo: (category: string) => void } = $props();

  let list = $state<ArchiveInfo[]>([]);
  let current = $state<number | null>(null);
  let items = $state<MemberInfo[]>([]);
  let filter = $state("");
  let changedOnly = $state(false);
  let error = $state<string | null>(null);
  let loadingMembers = $state(false);

  onMount(async () => {
    try {
      list = [...(await archives())].sort((a, b) => archiveGroup(a.id).localeCompare(archiveGroup(b.id)) || a.id - b.id);
    } catch (e) {
      error = String(e);
    }
  });

  async function load(id: number) {
    loadingMembers = true;
    try {
      const m = await members(id);
      // A slower answer for an archive left behind is dropped.
      if (current === id) items = m;
    } catch (e) {
      error = String(e);
    } finally {
      if (current === id) loadingMembers = false;
    }
  }

  function open(id: number) {
    play("select");
    current = id;
    items = [];
    changedOnly = false;
    error = null;
    load(id);
  }

  $effect(() => {
    void version;
    if (current !== null) load(current);
  });

  const query = debounced(() => filter);
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
    if (!q) return list;
    return list.filter(
      (a) =>
        a.path.includes(q) ||
        String(a.id) === q ||
        archiveLabel(a.id).toLowerCase().includes(q) ||
        archiveGroup(a.id).toLowerCase().includes(q),
    );
  });
  const info = $derived(list.find((a) => a.id === current));
  const hasChanged = $derived(items.some((m) => m.changed));
  const rows = $derived(changedOnly ? items.filter((m) => m.changed) : items);
  // The sidebar list that shows this archive properly, when there is one.
  const home = $derived(ALL.find((c) => "archive" in c && c.archive === current));

  const summary = (kinds: Record<string, number>) =>
    Object.entries(kinds)
      .sort((a, b) => b[1] - a[1])
      .map(([k, n]) => `${n} ${kindName(k).toLowerCase()}`)
      .join(", ");
  const size = (n: number) => (n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : `${(n / 1048576).toFixed(1)} MB`);
</script>

<div class="files">
  <aside class="archives">
    <input class="search" placeholder="Search {list.length || ''} game files" aria-label="Search game files" bind:value={filter} />
    <VirtualList items={shown} height={48} key={(a) => a.id} label="Game files">
      {#snippet row(a)}
        <button class="row-item" aria-current={a.id === current} title={archiveGroup(a.id)} onclick={() => open(a.id)}>
          <span class="two">
            <span class="name">{archiveLabel(a.id)}</span>
            <span class="muted path"><code>{a.path}</code></span>
          </span>
          <span class="tag" title="Entries">{a.members}</span>
        </button>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{list.length ? "Nothing matches." : error ? "" : "Reading the game files..."}</p>
      {/snippet}
    </VirtualList>
  </aside>
  <section class="members">
    {#if error}<p class="bad pad">{error}</p>{/if}
    {#if current === null || !info}
      <p class="muted pad">Choose a game file on the left.</p>
    {:else}
      <div class="head">
        <div class="title">
          <h2>{archiveLabel(info.id)}</h2>
          <code class="muted">{info.path}</code>
          <span class="spacer"></span>
          {#if home}
            <button onclick={() => onGo(home.id)}>Open in {home.label}</button>
          {/if}
        </div>
        {#if archiveNote(info.id)}<p class="muted">{archiveNote(info.id)}</p>{/if}
        <p class="muted">{summary(info.kinds)}</p>
        {#if hasChanged}
          <label class="check"><input type="checkbox" bind:checked={changedOnly} /> Changed only</label>
        {/if}
      </div>
      <div class="scroll">
        <VirtualList items={rows} height={34} key={(m) => m.index} label="Entries">
          {#snippet row(m)}
            <div class="row-item member">
              <strong class="num">#{m.index}</strong>
              <span class="label">{kindName(m.kind)}</span>
              <span class="muted">{size(m.size)}{m.compressed ? ", compressed" : ""}</span>
              {#if m.changed}<span class="tag changed">changed</span>{/if}
            </div>
          {/snippet}
          {#snippet empty()}
            <p class="muted pad">{loadingMembers ? "Loading..." : "Nothing here."}</p>
          {/snippet}
        </VirtualList>
      </div>
    {/if}
  </section>
</div>

<style>
  .files {
    display: grid;
    grid-template-columns: 18rem minmax(0, 1fr);
    height: 100%;
  }
  .archives {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.75rem;
    min-height: 0;
    min-width: 0;
    border-right: 2px solid var(--outline);
  }
  .two {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }
  .two > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    font-size: 0.78rem;
  }
  .row-item[aria-current="true"] .muted {
    color: inherit;
  }
  .members {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .head {
    padding: 0.6rem 1rem;
    border-bottom: 2px solid var(--outline);
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .head p {
    margin: 0;
  }
  .title {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 0.6rem;
  }
  .title h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  .spacer {
    flex: 1;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.9rem;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 0.5rem 1rem;
  }
  .member {
    cursor: default;
  }
  .member:hover {
    background: transparent;
  }
  .num {
    min-width: 4rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
