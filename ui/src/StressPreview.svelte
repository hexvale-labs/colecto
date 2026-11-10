<script lang="ts">
  import Mailbox from './extensions/Mailbox.svelte';
  import Reader from './extensions/Reader.svelte';
  import { demo, worst } from './lib/stress-fixtures';
  let mode = $state(new URLSearchParams(location.search).get('data') || 'Demo data');
  let selected = $state('');
  let messages = $derived(mode === 'Worst case' ? worst : mode === 'Empty' ? [] : mode === '100 rows' ? Array.from({ length: 100 }, (_, index) => ({ ...worst[index % worst.length], id: `${index}` })) : demo);
  function choose(next: string) { mode = next; selected = next === 'Worst case' ? 'long' : next === '100 rows' ? '0' : 'demo'; const url = new URL(location.href); url.searchParams.set('data', next); history.replaceState(null, '', url); }
</script>
<div class="app-shell stress-preview" class:detail-open={!!selected && messages.some(message => message.id === selected)}><main class="workspace"><div class="mailbox-column"><Mailbox items={messages.map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date }))} {selected} folder="Inbox" loading={false} error="" disabled={false} onopen={(id) => selected = id} onretry={() => {}} /></div><div class="reading-pane">{#if messages.some(message => message.id === selected)}{#key `${mode}:${selected}`}<Reader id={selected} folder="Inbox" accountLabel="International operations and customer experience" labels={{ '国際業務・要返信・International follow up': [selected] }} labeling={false} onlabel={() => {}} service={{ read: async (id) => messages.find(message => message.id === id) || messages[0], label: async () => ({}), content: async () => ({ text: '', html: null, attachments: [], message_id: '', references: '', reply_to: '', to: '', cc: '' }), attachment: async () => new Blob() }} onback={() => selected = ''} />{/key}{:else}<section class="welcome"><h2>Select a message</h2></section>{/if}</div></main></div>
<div class="stress-picker" role="group" aria-label="Preview data">{#each ['Demo data', 'Worst case', 'Empty', '100 rows'] as name}<button aria-pressed={mode === name} onclick={() => choose(name)}>{name}</button>{/each}</div>
