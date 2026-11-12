<script lang="ts" module>
  import type { IconName } from './Icon.svelte';
  export interface PaneInfo { id: string; title: string; icon: IconName }
</script>
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import Select from './Select.svelte';
  import { HEAD, MIN, dock, dockEdge, leaves, measure, preview, resize, setVisible, zoneIn, type Edge, type Gutter, type Layout, type Rect, type Zone } from './lib/layout';
  let { layout, panes, focus, compact, content, header, onactivate, onchange }: {
    layout: Layout; panes: PaneInfo[]; focus: string; compact: boolean; content: Snippet<[string]>; header?: Snippet<[string]>;
    onactivate: (pane: string) => void; onchange: (layout: Layout) => void;
  } = $props();
  type MenuAction = Edge | 'hide';
  const menu: { value: MenuAction; label: string }[] = [
    { value: 'left', label: 'Dock along the left edge' }, { value: 'right', label: 'Dock along the right edge' },
    { value: 'top', label: 'Dock along the top edge' }, { value: 'bottom', label: 'Dock along the bottom edge' },
    { value: 'hide', label: 'Hide this pane' },
  ];
  const EDGE = 14;
  let area = $state<HTMLElement>();
  let width = $state(0);
  let height = $state(0);
  let info = $derived(new Map(panes.map(pane => [pane.id, pane])));
  let groups = $derived(leaves(layout.root));
  let measured = $derived(measure(layout.root, { x: 0, y: 0, w: width, h: height }));
  let placed = $derived(groups.flatMap(group => group.panes.filter(pane => info.has(pane)).map(pane => ({ pane, group }))));
  type Target = { group: string; zone: Zone; index?: number } | { edge: Edge };
  let dragging = $state<{ pane: string; x: number; y: number; target: Target | null; shape: Rect | null } | null>(null);
  let resizing = $state<'row' | 'column' | null>(null);
  const box = (rect: Rect | undefined) => rect ? `left:${rect.x}px;top:${rect.y}px;width:${rect.w}px;height:${rect.h}px` : 'display:none';
  const body = (rect: Rect | undefined) => rect && box({ ...rect, y: rect.y + HEAD, h: rect.h - HEAD });

  function targetAt(x: number, y: number): { target: Target; shape: Rect } | null {
    if (x < 0 || y < 0 || x > width || y > height) return null;
    const edge: Edge | null = x < EDGE ? 'left' : x > width - EDGE ? 'right' : y < EDGE ? 'top' : y > height - EDGE ? 'bottom' : null;
    if (edge) return { target: { edge }, shape: preview({ x: 0, y: 0, w: width, h: height }, edge) };
    for (const [group, rect] of measured.groups) {
      if (x < rect.x || x > rect.x + rect.w || y < rect.y || y > rect.y + rect.h) continue;
      const zone = zoneIn(rect, x, y);
      let index: number | undefined;
      if (zone === 'center' && y < rect.y + HEAD) {
        const tabs = [...(area?.querySelectorAll<HTMLElement>(`[data-group="${group}"] .dock-tab`) ?? [])];
        const left = area?.getBoundingClientRect().left ?? 0;
        index = tabs.findIndex(tab => { const r = tab.getBoundingClientRect(); return x + left < r.left + r.width / 2; });
        if (index < 0) index = tabs.length;
      }
      return { target: { group, zone, index }, shape: preview(rect, zone) };
    }
    return null;
  }
  function startDrag(event: PointerEvent, pane: string) {
    if (event.button !== 0 || compact || !area) return;
    const start = { x: event.clientX, y: event.clientY };
    const origin = area.getBoundingClientRect();
    const moveTo = (e: PointerEvent) => {
      if (!dragging && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 6) return;
      const x = e.clientX - origin.left, y = e.clientY - origin.top;
      const hit = targetAt(x, y);
      dragging = { pane, x, y, target: hit?.target ?? null, shape: hit?.shape ?? null };
    };
    const stop = (e: PointerEvent | KeyboardEvent) => {
      window.removeEventListener('pointermove', moveTo); window.removeEventListener('pointerup', stop); window.removeEventListener('keydown', cancel);
      const target = dragging?.target;
      if (target && e.type === 'pointerup') onchange('edge' in target ? dockEdge(layout, pane, target.edge) : dock(layout, pane, target.group, target.zone, target.index));
      dragging = null;
    };
    const cancel = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.preventDefault(); stop(e); } };
    window.addEventListener('pointermove', moveTo); window.addEventListener('pointerup', stop); window.addEventListener('keydown', cancel);
  }

  function sizesFor(gutter: Gutter, sizes: number[], delta: number) {
    const next = [...sizes];
    const pair = (sizes[gutter.index] + sizes[gutter.index + 1]) * gutter.span;
    const min = Math.min(MIN[gutter.dir], pair / 2);
    const first = Math.max(min, Math.min(pair - min, sizes[gutter.index] * gutter.span + delta));
    next[gutter.index] = first / gutter.span; next[gutter.index + 1] = (pair - first) / gutter.span;
    return next;
  }
  function splitAt(path: number[]) {
    let node = layout.root;
    for (const index of path) node = node?.kind === 'split' ? node.children[index] : null;
    return node?.kind === 'split' ? node : null;
  }
  function startResize(event: PointerEvent, gutter: Gutter) {
    const split = splitAt(gutter.path);
    if (event.button !== 0 || !split) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    resizing = gutter.dir;
    const start = gutter.dir === 'row' ? event.clientX : event.clientY;
    const sizes = [...split.sizes];
    const update = (e: PointerEvent) => onchange(resize(layout, gutter.path, sizesFor(gutter, sizes, (gutter.dir === 'row' ? e.clientX : e.clientY) - start)));
    const stop = () => { handle.removeEventListener('pointermove', update); handle.removeEventListener('pointerup', stop); handle.removeEventListener('lostpointercapture', stop); resizing = null; };
    handle.addEventListener('pointermove', update); handle.addEventListener('pointerup', stop); handle.addEventListener('lostpointercapture', stop);
  }
  function resizeKey(event: KeyboardEvent, gutter: Gutter) {
    const split = splitAt(gutter.path);
    const keys = gutter.dir === 'row' ? { ArrowLeft: -1, ArrowRight: 1 } : { ArrowUp: -1, ArrowDown: 1 };
    const sign = keys[event.key as keyof typeof keys];
    if (!split || !sign) return;
    event.preventDefault();
    onchange(resize(layout, gutter.path, sizesFor(gutter, split.sizes, sign * (event.shiftKey ? 64 : 16))));
  }
  function equalize(gutter: Gutter) {
    const split = splitAt(gutter.path);
    if (!split) return;
    const sizes = [...split.sizes]; const half = (sizes[gutter.index] + sizes[gutter.index + 1]) / 2;
    sizes[gutter.index] = half; sizes[gutter.index + 1] = half;
    onchange(resize(layout, gutter.path, sizes));
  }
  function menuAction(pane: string, action: MenuAction) {
    onchange(action === 'hide' ? setVisible(layout, pane, false) : dockEdge(layout, pane, action));
  }
  const label = (gutter: Gutter) => gutter.dir === 'row' ? 'Resize the panes on either side' : 'Resize the panes above and below';
</script>

<div bind:this={area} bind:clientWidth={width} bind:clientHeight={height} class="dock-area" class:compact class:dragging={!!dragging} data-resizing={resizing}>
  {#if !compact}
    {#each groups as group (group.id)}
      {@const current = info.get(group.active)}
      <section class="dock-frame" data-group={group.id} style={box(measured.groups.get(group.id))} aria-label={`${current?.title ?? 'Pane'} group`}>
        <header class="dock-head">
          <div class="dock-tabs" role="tablist" aria-label="Panes in this group">
            {#each group.panes as pane (pane)}
              {@const meta = info.get(pane)}
              {#if meta}<button role="tab" id={`dock-tab-${pane}`} class="dock-tab" aria-selected={group.active === pane} aria-controls={`pane-${pane}`} title="Drag onto another pane to dock it there" onpointerdown={(event) => startDrag(event, pane)} onclick={() => onactivate(pane)}><Icon name={meta.icon} size={14} /><span>{meta.title}</span></button>{/if}
            {/each}
          </div>
          {#if header}{@render header(group.active)}{/if}
          {#if current}<div class="dock-move"><Select label={`Arrange ${current.title}`} value={'' as MenuAction} options={menu} onchange={(action) => menuAction(current.id, action)}>{#snippet trigger()}<Icon name="layout" size={15} />{/snippet}</Select></div>{/if}
        </header>
      </section>
    {/each}
    {#each measured.gutters as gutter (`${gutter.path.join('.')}:${gutter.index}`)}
      {@const split = splitAt(gutter.path)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (a focusable separator is the ARIA splitter pattern) -->
      <div class="dock-gutter {gutter.dir}" style={box(gutter.rect)} role="separator" tabindex="0" aria-label={label(gutter)} aria-orientation={gutter.dir === 'row' ? 'vertical' : 'horizontal'}
        aria-valuenow={Math.round((split?.sizes[gutter.index] ?? 0) * 100)} aria-valuemin={0} aria-valuemax={100}
        onpointerdown={(event) => startResize(event, gutter)} onkeydown={(event) => resizeKey(event, gutter)} ondblclick={() => equalize(gutter)}></div>
    {/each}
  {/if}
  {#each placed as { pane, group } (pane)}
    <div class="pane" id={`pane-${pane}`} role={compact ? undefined : 'tabpanel'} aria-labelledby={compact ? undefined : `dock-tab-${pane}`} style={compact ? undefined : body(measured.groups.get(group.id))} hidden={compact ? pane !== focus : group.active !== pane}>{@render content(pane)}</div>
  {/each}
  {#if dragging}
    <div class="drop-overlay" aria-hidden="true">
      {#if dragging.shape}<div class="drop-preview" style={box(dragging.shape)}></div>{/if}
      <span class="drag-ghost" style:transform={`translate(${Math.min(dragging.x + 12, width - 150)}px, ${Math.min(dragging.y + 10, height - 34)}px)`}><Icon name={info.get(dragging.pane)?.icon ?? 'layout'} size={14} />{info.get(dragging.pane)?.title}</span>
    </div>
  {/if}
</div>
