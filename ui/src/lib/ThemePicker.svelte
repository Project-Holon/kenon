<script lang="ts">
  import { THEMES, savedTheme, setTheme, type Theme } from "./theme";
  import { play } from "./sfx";
  import { asset } from "./backend";
  import { once } from "./cache.svelte";

  // With a ROM open the themes are picked by their legendary's party icon,
  // read from the ROM: Zekrom for night, Reshiram for day, Kyurem for the
  // system setting. Without one, a plain list.
  let { gameOpen = false }: { gameOpen?: boolean } = $props();

  const ICONS: Record<Theme, number> = { dark: 644, light: 643, system: 646 };

  let theme = $state<Theme>(savedTheme());
  let palettes = $state<number[] | null>(null);
  let bounce = $state(0);

  // The bounce of the selected icon: its two frames, 8/60 s each, as the
  // party screen plays them. Only while the icons show and the window is
  // in view, and not for people who ask for less motion.
  $effect(() => {
    if (!gameOpen || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const timer = setInterval(() => {
      if (!document.hidden) bounce = 1 - bounce;
    }, 133);
    return () => clearInterval(timer);
  });

  $effect(() => {
    if (!gameOpen) return;
    once("table/icon_palette", () => fetch(asset("table/icon_palette")).then((r) => (r.ok ? r.json() : [])))
      .then((t: number[]) => (palettes = t.length ? t : null))
      .catch(() => (palettes = null));
  });

  function choose(t: Theme) {
    theme = t;
    setTheme(t);
    play("select");
  }

  const icon = (species: number, frame: number) =>
    asset(`anim/7:${7 + 2 * species}/7:0/7:2/7:1/1/${frame}?paloffset=${(palettes?.[species] ?? 0) & 0xf}`);
</script>

{#if gameOpen}
  <div class="icons" role="radiogroup" aria-label="Theme">
    {#each THEMES as [value, label]}
      <button
        class="icon"
        class:on={theme === value}
        role="radio"
        aria-checked={theme === value}
        title={label}
        aria-label={label}
        onclick={() => choose(value)}
      >
        <img class="px" src={icon(ICONS[value], theme === value ? bounce : 0)} alt="" />
      </button>
    {/each}
  </div>
{:else}
  <label class="picker">
    <span class="muted">Theme</span>
    <select value={theme} onchange={(e) => choose((e.currentTarget as HTMLSelectElement).value as Theme)}>
      {#each THEMES as [value, label]}<option {value}>{label}</option>{/each}
    </select>
  </label>
{/if}

<style>
  .picker {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .icons {
    display: flex;
    gap: 0.2rem;
  }
  .icon {
    width: 40px;
    height: 40px;
    padding: 0;
    display: grid;
    place-items: center;
    background: transparent;
    border-color: transparent;
    box-shadow: none;
    opacity: 0.55;
  }
  .icon:hover {
    opacity: 0.9;
  }
  .icon.on {
    opacity: 1;
    background: var(--panel-2);
    border-color: var(--accent);
  }
  .icon img {
    width: 32px;
    height: 32px;
  }
</style>
