// Global layers; local workspace layers stay inside its stacking context.
export const layers = { workspace: 0, thirdPartyDefault: 10, bundledOverlay: 500,
  settings: 1000, menus: 1100, commandRunner: 1200, notifications: 1250, installDialog: 1300,
  content: 1, toolbar: 3, gutter: 4, expandedPane: 6, drag: 20, scrim: 1099 } as const;
export function installLayers() {
  for (const [name, value] of Object.entries(layers)) document.documentElement.style.setProperty(`--layer-${name}`, String(value));
}
