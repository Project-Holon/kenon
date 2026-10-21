<script lang="ts">
  import { THEMES, savedTheme, setTheme, type Theme } from "./theme";
  import { play } from "./sfx";

  let theme = $state<Theme>(savedTheme());
</script>

<label class="picker">
  <span class="muted">Theme</span>
  <select
    value={theme}
    onchange={(e) => {
      theme = (e.currentTarget as HTMLSelectElement).value as Theme;
      setTheme(theme);
      play("select");
    }}
  >
    {#each THEMES as [value, label]}<option {value}>{label}</option>{/each}
  </select>
</label>

<style>
  .picker {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
</style>
