<script lang="ts" module>
  import type { IconName } from './Icon.svelte';
  export interface PaneInfo { id: string; title: string; icon: IconName }
</script>
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import Select from './Select.svelte';
  import { docks, minSize, move, type Dock, type Layout } from './lib/layout';
  let { layout, panes, focus, compact, content, onactivate, onchange }: {
    layout: Layout; panes: PaneInfo[]; focus: string; compact: boolean; content: Snippet<[string]>;
    onactivate: (pane: string) => void; onchange: (layout: Layout) => void;
  } = $props();
  const names: Record<Dock, string> = { left: 'Left', center: 'Center', right: 'Right', bottom: 'Bottom' };
  const dockOptions = docks.map(dock => ({ value: dock, label: `Dock ${names[dock].toLowerCase()}` }));
  let area = $state<HTMLElement>();
  let dragging = $state<{ pane: string; zone: Dock | null } | null>(null);
  let resizing = $state<Dock | null>(null);
  let info = $derived(new Map(panes.map(pane => [pane.id, pane])));
  let placed = $derived(docks.flatMap(dock => layout.docks[dock].filter(pane => info.has(pane)).map(pane => ({ pane, dock }))));
  let used = $derived(docks.filter(dock => placed.some(item => item.dock === dock)));
  // Side docks keep their size; whichever top dock is last absorbs the remaining width when the center is empty.
  let grid = $derived.by(() => {
    const top = (['left', 'center', 'right'] as const).filter(dock => used.includes(dock));
    const columns = top.map(dock => dock === 'center' ? 'minmax(0, 1fr)' : `min(${layout.size[dock]}px, 44%)`);
    if (top.length && !top.includes('center')) columns[columns.length - 1] = 'minmax(0, 1fr)';
    const bottom = used.includes('bottom');
    const width = Math.max(top.length, 1);
    return {
      columns: columns.join(' ') || 'minmax(0, 1fr)',
      rows: !top.length ? 'minmax(0, 1fr)' : bottom ? `minmax(0, 1fr) min(${layout.size.bottom}px, 60%)` : 'minmax(0, 1fr)',
      areas: [top.length ? `"${top.join(' ')}"` : '', bottom ? `"${Array(width).fill('bottom').join(' ')}"` : ''].filter(Boolean).join(' '),
    };
  });
  const visible = (pane: string, dock: Dock) => compact ? pane === focus : layout.active[dock] === pane;

  function zoneAt(x: number, y: number): Dock | null {
    if (!area) return null;
    const rect = area.getBoundingClientRect();
    const rx = (x - rect.left) / rect.width, ry = (y - rect.top) / rect.height;
    if (rx < 0 || rx > 1 || ry < 0 || ry > 1) return null;
    return ry > 0.72 ? 'bottom' : rx < 0.24 ? 'left' : rx > 0.76 ? 'right' : 'center';
  }
  function startDrag(event: PointerEvent, pane: string) {
    if (event.button !== 0 || compact) return;
    const start = { x: event.clientX, y: event.clientY };
    const moveTo = (e: PointerEvent) => {
      if (!dragging && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 6) return;
      dragging = { pane, zone: zoneAt(e.clientX, e.clientY) };
    };
    const stop = (e: PointerEvent | KeyboardEvent) => {
      window.removeEventListener('pointermove', moveTo); window.removeEventListener('pointerup', stop); window.removeEventListener('keydown', cancel);
      if (dragging?.zone && e.type === 'pointerup') onchange(move(layout, dragging.pane, dragging.zone));
      dragging = null;
    };
    const cancel = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.preventDefault(); stop(e); } };
    window.addEventListener('pointermove', moveTo); window.addEventListener('pointerup', stop); window.addEventListener('keydown', cancel);
  }

  function clamp(dock: 'left' | 'right' | 'bottom', value: number) {
    const rect = area?.getBoundingClientRect();
    const room = dock === 'bottom' ? (rect?.height ?? 800) - 160 : (rect?.width ?? 1200) - 320;
    return Math.round(Math.max(minSize[dock], Math.min(value, room)));
  }
  function resize(dock: 'left' | 'right' | 'bottom', value: number) {
    onchange({ ...layout, size: { ...layout.size, [dock]: clamp(dock, value) } });
  }
  function startResize(event: PointerEvent, dock: 'left' | 'right' | 'bottom') {
    if (event.button !== 0 || !area) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    resizing = dock;
    const rect = area.getBoundingClientRect();
    const update = (e: PointerEvent) => resize(dock, dock === 'left' ? e.clientX - rect.left : dock === 'right' ? rect.right - e.clientX : rect.bottom - e.clientY);
    const stop = () => { handle.removeEventListener('pointermove', update); handle.removeEventListener('pointerup', stop); handle.removeEventListener('lostpointercapture', stop); resizing = null; };
    handle.addEventListener('pointermove', update); handle.addEventListener('pointerup', stop); handle.addEventListener('lostpointercapture', stop);
  }
  function resizeKey(event: KeyboardEvent, dock: 'left' | 'right' | 'bottom') {
    const grow = dock === 'left' ? 'ArrowRight' : dock === 'right' ? 'ArrowLeft' : 'ArrowUp';
    const shrink = dock === 'left' ? 'ArrowLeft' : dock === 'right' ? 'ArrowRight' : 'ArrowDown';
    const step = event.shiftKey ? 64 : 16;
    if (event.key === grow) resize(dock, layout.size[dock] + step);
    else if (event.key === shrink) resize(dock, layout.size[dock] - step);
    else return;
    event.preventDefault();
  }
</script>

<div bind:this={area} class="dock-area" class:compact class:dragging={!!dragging} class:resizing={!!resizing} data-resizing={resizing}
  style:grid-template-columns={compact ? undefined : grid.columns} style:grid-template-rows={compact ? undefined : grid.rows} style:grid-template-areas={compact ? undefined : grid.areas}>
  {#if !compact}
    {#each used as dock (dock)}
      {@const items = layout.docks[dock].filter(pane => info.has(pane))}
      {@const current = info.get(layout.active[dock]) ?? info.get(items[0])}
      <section class="dock-frame" style:grid-area={dock} aria-label={`${names[dock]} dock`}>
        <header class="dock-head">
          <div class="dock-tabs" role="tablist" aria-label={`${names[dock]} dock panes`}>
            {#each items as pane (pane)}
              {@const meta = info.get(pane)}
              {#if meta}<button role="tab" id={`dock-tab-${pane}`} class="dock-tab" aria-selected={layout.active[dock] === pane} aria-controls={`pane-${pane}`} title="Drag to another dock" onpointerdown={(event) => startDrag(event, pane)} onclick={() => onactivate(pane)}><Icon name={meta.icon} size={14} /><span>{meta.title}</span></button>{/if}
            {/each}
          </div>
          {#if current}<div class="dock-move"><Select label={`Move ${current.title}`} value={dock} options={dockOptions} onchange={(to) => onchange(move(layout, current.id, to))}>{#snippet trigger()}<Icon name="layout" size={15} />{/snippet}</Select></div>{/if}
        </header>
        {#if dock !== 'center'}
          {@const side = dock}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (a focusable separator is the ARIA splitter pattern) -->
          <div class="dock-resize {side}" role="separator" tabindex="0" aria-label={`Resize ${names[side].toLowerCase()} dock`} aria-orientation={side === 'bottom' ? 'horizontal' : 'vertical'} aria-valuenow={layout.size[side]} aria-valuemin={minSize[side]}
            onpointerdown={(event) => startResize(event, side)} onkeydown={(event) => resizeKey(event, side)} ondblclick={() => resize(side, side === 'bottom' ? 260 : side === 'left' ? 344 : 360)}></div>
        {/if}
      </section>
    {/each}
  {/if}
  {#each placed as { pane, dock } (pane)}
    <div class="pane" id={`pane-${pane}`} role={compact ? undefined : 'tabpanel'} aria-labelledby={compact ? undefined : `dock-tab-${pane}`} style:grid-area={compact ? undefined : dock} data-dock={dock} hidden={!visible(pane, dock)}>{@render content(pane)}</div>
  {/each}
  {#if dragging}
    <div class="drop-overlay" aria-hidden="true">
      {#if dragging.zone}<div class="drop-preview {dragging.zone}"><span>Dock {names[dragging.zone].toLowerCase()}</span></div>{/if}
    </div>
  {/if}
</div>
