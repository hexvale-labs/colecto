<script lang="ts">
  import mark from './assets/logo-square.svg';
  import type { Corner } from './lib/api';
  let { value, disabled = false, onchange }: { value: Corner; disabled?: boolean; onchange: (corner: Corner) => void } = $props();
  // Clockwise from the top left, so arrow keys walk around the frame.
  const corners: { value: Corner; label: string }[] = [
    { value: 'top-left', label: 'Top left' }, { value: 'top', label: 'Top center' }, { value: 'top-right', label: 'Top right' },
    { value: 'right', label: 'Right' }, { value: 'bottom-right', label: 'Bottom right' }, { value: 'bottom', label: 'Bottom center' },
    { value: 'bottom-left', label: 'Bottom left' }, { value: 'left', label: 'Left' },
  ];
</script>

<!-- A drawn stand-in for the window, not a screenshot, so it never goes stale as the app changes. -->
<fieldset class="corner-picker" {disabled}>
  <legend class="sr-only">Notification position</legend>
  <div class="corner-screen">
    <img src={mark} alt="" width="34" height="34" draggable="false" />
    {#each corners as corner (corner.value)}
      <label class="corner-spot" data-corner={corner.value} title={corner.label}>
        <input type="radio" name="notification-position" value={corner.value} checked={value === corner.value} onchange={() => onchange(corner.value)} />
        <span class="sr-only">{corner.label}</span>
      </label>
    {/each}
  </div>
</fieldset>
