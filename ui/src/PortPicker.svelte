<script lang="ts">
  // One joined control: the usual ports as choices, plus a box for any other port.
  let { label, value, options, onpick, id }: {
    label: string; value: number; id?: string;
    /** The usual ports for this server, most common first; `hint` names what each one means. */
    options: { port: number; hint: string }[];
    /** Called with every valid port the owner picks or types. */
    onpick: (port: number) => void;
  } = $props();
  const uid = $props.id();
  const name = `port-${uid}`;
  let preset = $derived(options.some(option => option.port === value));
  let typing = $state(false);
  let draft = $state('');
  // While the owner types (or left an impossible port behind), keep their text; otherwise mirror the current port.
  let shown = $derived(typing ? draft : preset ? '' : String(value));
  let input = $state<HTMLInputElement>();

  function type(event: Event & { currentTarget: HTMLInputElement }) {
    const text = event.currentTarget.value.replace(/\D/g, '').slice(0, 5);
    draft = text;
    event.currentTarget.value = text;
    const port = Number(text);
    const valid = text !== '' && port >= 1 && port <= 65535;
    event.currentTarget.setCustomValidity(text === '' || valid ? '' : 'Enter a port from 1 to 65535.');
    if (valid) onpick(port);
  }
</script>

<fieldset class="port-picker">
  <legend>{label}</legend>
  <div class="port-options" class:custom={!preset}>
    {#each options as option (option.port)}
      <label class="port-option" title={option.hint}>
        <input type="radio" {name} checked={value === option.port} onchange={() => { typing = false; onpick(option.port); }} />
        <span>{option.port}</span><span class="sr-only">, {option.hint}</span>
      </label>
    {/each}
    <label class="port-other" class:active={!preset}>
      <span class="sr-only">Other {label}</span>
      <input bind:this={input} {id} type="text" inputmode="numeric" autocomplete="off" maxlength="5" placeholder="Other…" value={shown}
        onfocus={() => { typing = true; draft = shown; }} onblur={() => { if (!input?.validationMessage) typing = false; }} oninput={type} />
    </label>
  </div>
</fieldset>

<style>
  .port-picker { min-width: 0; padding: 0; margin: 0; border: 0; }
  legend { padding: 0; margin-bottom: 6px; color: var(--ink-2); font-size: calc(12.5px * var(--text-scale)); font-weight: 560; }
  .port-options { display: flex; height: 38px; padding: 3px; gap: 2px; border: 1px solid var(--line); border-radius: 9px; background: var(--sunk); }
  .port-option, .port-other { position: relative; display: grid; place-items: center; min-width: 0; border-radius: 6px; color: var(--ink-2); font-size: calc(13px * var(--text-scale)); font-variant-numeric: tabular-nums; transition: background-color 140ms ease, color 140ms ease; }
  .port-option { flex: 1 1 0; padding: 0 8px; cursor: pointer; }
  .port-option input { position: absolute; opacity: 0; pointer-events: none; }
  .port-option:has(input:checked), .port-other.active { background: var(--control); color: var(--ink); }
  .port-option:has(input:focus-visible) { outline: 2px solid var(--mint); outline-offset: 1px; }
  /* The free-entry box takes the room the presets leave, like the last cell of the sketch. */
  .port-other { flex: 1.6 1 0; }
  .port-other input { width: 100%; height: 100%; padding: 0 8px; border: 0; border-radius: 6px; background: transparent; color: inherit; font: inherit; text-align: center; outline: none; }
  .port-other input::placeholder { color: var(--ink-3); }
  .port-other:focus-within { box-shadow: inset 0 0 0 1px var(--mint); }
  .port-other input:user-invalid { color: var(--danger); }
  @media (hover: hover) and (pointer: fine) { .port-option:hover:not(:has(input:checked)) { color: var(--ink); } }
</style>
