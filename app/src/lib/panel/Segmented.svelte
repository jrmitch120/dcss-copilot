<script lang="ts" generics="T extends string">
/** A row of buttons choosing one of `choices`, each a value and its label, with `chosen` pressed. */
let {
  label,
  choices,
  chosen,
  choose,
}: { label: string; choices: [T, string][]; chosen: T; choose: (choice: T) => void } = $props();
</script>

<fieldset class="seg" aria-label={label}>
  {#each choices as [choice, text] (choice)}
    <button
      type="button"
      class:on={chosen === choice}
      aria-pressed={chosen === choice}
      onclick={() => choose(choice)}
    >
      {text}
    </button>
  {/each}
</fieldset>

<style>
.seg {
  display: flex;
  margin: 0;
  border: 0;
  padding: 2px;
  border-radius: 8px;
  background: var(--surface);
}

.seg button {
  padding: 2px 8px;
  border: 0;
  border-radius: 6px;
  background: none;
  font-size: 11px;
  color: var(--text-muted);
  cursor: pointer;
}

.seg button.on {
  background: rgba(255, 255, 255, 0.9);
  color: #111111;
  font-weight: 600;
}
</style>
