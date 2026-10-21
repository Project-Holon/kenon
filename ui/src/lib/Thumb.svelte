<script lang="ts">
  import { thumb } from "./thumbs";

  // A picture loaded through the thumbnail queue.
  let { src, alt = "", class: cls = "" }: { src: string; alt?: string; class?: string } = $props();

  let url = $state<string | null>(null);

  $effect(() => {
    const controller = new AbortController();
    url = null;
    // A picture that arrives after the row moved on is ignored.
    if (src) thumb(src, controller.signal).then((u) => !controller.signal.aborted && (url = u)).catch(() => {});
    return () => controller.abort();
  });
</script>

{#if url}
  <img class="px {cls}" src={url} {alt} draggable="false" />
{:else}
  <span class="placeholder {cls}"></span>
{/if}

<style>
  .placeholder {
    display: block;
    width: 24px;
    height: 24px;
    border-radius: 4px;
    background: var(--panel-3);
    opacity: 0.5;
  }
</style>
