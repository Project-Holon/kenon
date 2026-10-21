<script lang="ts">
  let { label }: { label: string } = $props();
</script>

<div class="pending">
  <div class="window note">
    <h2>{label}</h2>
    <p class="muted">
      The data tables (Pokemon stats, moves, items, learnsets, evolutions, trainers, encounters) are being
      mapped from the decompiled game now. This editor comes next: every value as a number field or a
      list, checked against the game's own limits.
    </p>
  </div>
</div>

<style>
  .pending {
    height: 100%;
    display: grid;
    place-items: center;
    padding: 2rem;
  }
  .note {
    max-width: 32rem;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
</style>
