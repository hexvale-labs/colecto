<script lang="ts" module>
  import type { AccountInfo, Preferences } from './lib/api';
  /** First run: no real account yet and onboarding never finished. The demo account never counts. */
  export const needsOnboarding = (accounts: AccountInfo[], preferences: Pick<Preferences, 'onboarded'> | null) =>
    !!preferences && !preferences.onboarded && !accounts.some(account => account.settings);
</script>
<script lang="ts">
  // First-run setup. It stays in the shell rather than an extension: it is bootstrapping and handles credentials.
  import { onMount, tick } from 'svelte';
  import AccountForm from './AccountForm.svelte';
  import Icon from './Icon.svelte';
  import mark from './assets/logo-square.svg';
  import { hostService } from './lib/api';
  let { ondone }: { ondone: () => void } = $props();
  type Step = 'welcome' | 'account' | 'done';
  let step = $state<Step>('welcome');
  let saving = $state(false);
  let finishing = $state(false);
  let added = $state('');
  let error = $state('');
  let heading = $state<HTMLElement>();
  let addedId = '';
  async function go(next: Step) { step = next; await tick(); heading?.focus(); }
  async function saved(account: AccountInfo, name: string) { added = name; addedId = account.id; await go('done'); }
  async function finish(demo = false) {
    finishing = true; error = '';
    try {
      await hostService.savePreferences(demo ? { onboarded: true, accounts_enabled: { demo: true }, account: 'demo' } : { onboarded: true, ...(addedId ? { account: addedId } : {}) });
      ondone();
    } catch (e) { error = (e as Error).message; finishing = false; }
  }
  onMount(() => { heading?.focus(); });
</script>

<main class="onboarding" aria-labelledby="onboarding-title">
  <div class="onboarding-glow" aria-hidden="true"></div>
  {#key step}
    <section class="onboarding-card" class:wide={step === 'account'}>
      {#if step === 'welcome'}
        <div class="mark-wrap enter" style:--i="0"><img class="mark" src={mark} alt="" width="64" height="64" /></div>
        <h1 id="onboarding-title" class="enter" style:--i="1" tabindex="-1" bind:this={heading}>Welcome to Colecto</h1>
        <p class="lede enter" style:--i="2">A calm mail client for IMAP and JMAP. Add your first account to get started; your password stays on this device.</p>
        <div class="actions enter" style:--i="3">
          <button class="primary go" onclick={() => go('account')}>Add your first account<Icon name="arrow-right" size={16} /></button>
          <button class="text-button" disabled={finishing} onclick={() => finish(true)}>Explore with sample mail first</button>
        </div>
        <ol class="steps enter" style:--i="4" aria-label="Setup steps"><li class="current">Welcome</li><li>Account</li><li>Inbox</li></ol>
      {:else if step === 'account'}
        <header class="account-head enter" style:--i="0">
          <button class="icon-button" aria-label="Back" disabled={saving} onclick={() => go('welcome')}><Icon name="back" size={18} /></button>
          <div><h1 id="onboarding-title" tabindex="-1" bind:this={heading}>Add your account</h1><p>Use your provider’s server settings, and an app password if it asks for one.</p></div>
        </header>
        <div class="enter" style:--i="1"><AccountForm bind:working={saving} submitLabel="Connect account" onsaved={saved} /></div>
      {:else}
        <svg class="done-mark" viewBox="0 0 64 64" aria-hidden="true"><circle class="ring" cx="32" cy="32" r="28" /><path class="tick" d="M20 33.5l8 8 16-17" /></svg>
        <h1 id="onboarding-title" class="enter" style:--i="1" tabindex="-1" bind:this={heading}>You’re all set</h1>
        <p class="lede enter" style:--i="2"><strong>{added}</strong> is added. Colecto fetches your mail in the background; it shows up in your Inbox as it arrives.</p>
        <div class="actions enter" style:--i="3"><button class="primary go" disabled={finishing} onclick={() => finish()}>{finishing ? 'Opening…' : 'Open Inbox'}<Icon name="arrow-right" size={16} /></button></div>
      {/if}
      {#if error}<p class="error-box" role="alert">{error}</p>{/if}
    </section>
  {/key}
</main>

<style>
  .onboarding { position: fixed; inset: 0; display: grid; place-items: center; padding: 24px 16px; overflow-y: auto; background: var(--ground); }
  /* A faint accent wash behind the card; it drifts very slowly so the screen feels alive without moving anything you read. */
  .onboarding-glow { position: fixed; left: 50%; top: 38%; width: min(720px, 120vw); aspect-ratio: 1; translate: -50% -50%; background: radial-gradient(closest-side, color-mix(in oklch, var(--mint) 9%, transparent), transparent); pointer-events: none; animation: drift 14s ease-in-out infinite alternate; }
  .onboarding-card { position: relative; display: grid; justify-items: center; gap: 14px; width: min(440px, 100%); text-align: center; }
  .onboarding-card.wide { width: min(560px, 100%); justify-items: stretch; text-align: left; padding: 0; border: 1px solid var(--line); border-radius: 16px; background: var(--float); box-shadow: 0 30px 80px -20px oklch(0 0 0 / 0.75); overflow: hidden; }
  .onboarding-card.wide :global(.sheet-body) { max-height: min(560px, calc(100dvh - 220px)); }
  h1 { font-size: calc(26px * var(--text-scale)); font-stretch: 106%; outline: none; }
  .wide h1 { font-size: calc(19px * var(--text-scale)); }
  .lede { max-width: 38ch; color: var(--ink-2); line-height: 1.55; }
  .lede strong { color: var(--ink); font-weight: 600; }
  .mark-wrap { position: relative; margin-bottom: 6px; }
  .mark { display: block; border-radius: 16px; animation: float 6s ease-in-out infinite; }
  /* A soft halo breathes behind the icon. */
  .mark-wrap::after { content: ''; position: absolute; inset: -10px; z-index: -1; border-radius: 24px; background: color-mix(in oklch, var(--mint) 16%, transparent); filter: blur(14px); animation: breathe 6s ease-in-out infinite; }
  .actions { display: grid; justify-items: center; gap: 10px; margin-top: 10px; }
  .go { height: 40px; padding: 0 18px; font-size: calc(14px * var(--text-scale)); transition: transform 160ms var(--ease-out), box-shadow 220ms var(--ease-out), background-color 140ms ease; }
  .go :global(svg) { transition: transform 200ms var(--ease-out); }
  @media (hover: hover) and (pointer: fine) {
    .go:hover:not(:disabled) { transform: translateY(-1px); box-shadow: 0 10px 28px -12px color-mix(in oklch, var(--mint) 70%, transparent); }
    .go:hover:not(:disabled) :global(svg) { transform: translateX(3px); }
  }
  .go:active:not(:disabled) { transform: scale(0.97); }
  .steps { display: flex; gap: 18px; margin-top: 18px; color: var(--ink-3); font-size: calc(12px * var(--text-scale)); }
  .steps li { display: flex; align-items: center; gap: 6px; }
  .steps li::before { content: ''; width: 6px; height: 6px; border-radius: 50%; background: var(--line); }
  .steps li.current { color: var(--ink-2); }
  .steps li.current::before { background: var(--mint); }
  .account-head { display: flex; align-items: flex-start; gap: 8px; padding: 18px 20px 4px 12px; }
  .account-head p { margin-top: 4px; color: var(--ink-2); font-size: calc(13.5px * var(--text-scale)); line-height: 1.5; }
  .done-mark { width: 64px; height: 64px; margin-bottom: 6px; fill: none; stroke: var(--mint); stroke-width: 3; stroke-linecap: round; stroke-linejoin: round; }
  .done-mark .ring { stroke-dasharray: 176; stroke-dashoffset: 176; opacity: 0.45; animation: draw 700ms var(--ease-out) forwards; }
  .done-mark .tick { stroke-dasharray: 40; stroke-dashoffset: 40; animation: draw 420ms var(--ease-out) 380ms forwards; }
  /* Content arrives in a short stagger on every step. */
  .enter { animation: rise 520ms var(--ease-out) both; animation-delay: calc(var(--i, 0) * 70ms); }
  .error-box { margin: 0 20px 16px; text-align: left; }
  @keyframes rise { from { opacity: 0; transform: translateY(8px); filter: blur(2px); } }
  @keyframes float { 50% { transform: translateY(-4px) rotate(-1.5deg); } }
  @keyframes breathe { 50% { opacity: 0.55; transform: scale(1.06); } }
  @keyframes drift { to { translate: -46% -54%; } }
  @keyframes draw { to { stroke-dashoffset: 0; } }
  @media (prefers-reduced-motion: reduce) {
    .onboarding-glow, .mark, .mark-wrap::after { animation: none; }
    .enter { animation-duration: 1ms; animation-delay: 0ms; }
    .done-mark .ring, .done-mark .tick { animation-duration: 1ms; animation-delay: 0ms; }
  }
</style>
