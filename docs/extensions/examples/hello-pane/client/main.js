// A complete client part without any library: it runs in a Web Worker, receives shell
// messages through `colecto.onmessage` and answers with `colecto.send`. See
// docs/extensions/ui-model.md for the protocol and docs/extensions/guide.md for a walkthrough.
let account = null;
let greeting = 'Hello';
let presses = 0;
let connected = null;

function render() {
  colecto.send({ t: 'render', surface: 'pane', tree: {
    type: 'section', label: 'Hello pane', children: [
      { type: 'heading', level: 2, text: `${greeting}, ${account ? account.name || account.address || 'Demo account' : 'nobody'}` },
      { type: 'text', id: 'status', tone: 'muted', text: connected === null ? 'Connection not checked yet.' : connected ? 'Connected.' : 'Not connected.' },
      { type: 'stack', direction: 'row', gap: 8, children: [
        { type: 'button', id: 'press', label: `Pressed ${presses} ${presses === 1 ? 'time' : 'times'}`, variant: 'secondary' },
        { type: 'button', id: 'check', label: 'Check connection', variant: 'tool', icon: 'refresh' },
      ] },
    ],
  } });
}

let calls = 0;
const pending = new Map();
function call(method, path) {
  const id = ++calls;
  colecto.send({ t: 'call', id, method, path });
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

colecto.onmessage = async (message) => {
  switch (message.t) {
    case 'start':
      account = message.account;
      greeting = message.settings.greeting || 'Hello';
      render();
      break;
    case 'account':
      account = message.account; connected = null; render();
      break;
    case 'settings':
      greeting = message.settings.greeting || 'Hello'; render();
      break;
    case 'event':
      if (message.node === 'press') { presses += 1; render(); }
      if (message.node === 'check') {
        try { connected = await call('GET', '/account'); } catch { connected = false; }
        render();
      }
      break;
    case 'reply': {
      const waiting = pending.get(message.id);
      pending.delete(message.id);
      if (message.ok) waiting?.resolve(message.data); else waiting?.reject(new Error(message.error));
      break;
    }
    case 'ping':
      colecto.send({ t: 'pong', n: message.n });
      break;
  }
};
