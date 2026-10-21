<script lang="ts">
  import Settings from "./screens/Settings.svelte";
  import Home from "./screens/Home.svelte";

  type Screen = "home" | "settings";
  let screen = $state<Screen>("home");
</script>

<div class="shell">
  <nav aria-label="Screens">
    <span class="brand">kenon</span>
    <button aria-current={screen === "home"} onclick={() => (screen = "home")}>Home</button>
    <button aria-current={screen === "settings"} onclick={() => (screen = "settings")}>Settings</button>
  </nav>
  <main>
    {#if screen === "home"}
      <Home onOpenSettings={() => (screen = "settings")} />
    {:else}
      <Settings />
    {/if}
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 11rem 1fr;
    min-height: 100vh;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 1rem 0.75rem;
    background: var(--surface-2);
    border-right: 1px solid var(--border);
  }
  .brand {
    font-weight: 700;
    padding: 0 0.5rem 0.75rem;
  }
  nav button {
    text-align: left;
    background: transparent;
    border-color: transparent;
  }
  nav button[aria-current="true"] {
    background: var(--surface);
    border-color: var(--border);
  }
  main {
    padding: 1.5rem 2rem;
    max-width: 52rem;
  }
</style>
