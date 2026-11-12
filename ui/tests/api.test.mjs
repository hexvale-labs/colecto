import { afterEach, expect, test } from 'bun:test';
import { createServices, hostService, ServiceError } from '../src/lib/api.ts';

const originalFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = originalFetch; });

for (const failure of ['malformed', 'interrupted']) {
  for (const operation of ['send', 'approve']) {
    test(`${operation} reports uncertainty after ${failure} successful response`, async () => {
      globalThis.fetch = async () => failure === 'malformed'
        ? new Response('<html>Proxy reply</html>', { status: 200 })
        : new Response(new ReadableStream({ start(controller) { controller.error(new TypeError('Connection closed')); } }), { status: 200 });
      const result = operation === 'send'
        ? createServices('demo').composer.send({ recipient: 'friend@example.test', subject: 'Hello', body: 'Preserve this draft' })
        : hostService.decideSend('request-id', 'approve');
      const error = await result.catch(error => error);
      expect(error).toBeInstanceOf(ServiceError);
      expect(error.category).toBe('delivery_uncertain');
      expect(error.message).toContain('Check Sent');
      expect(error.message).not.toContain('try again');
    });
  }
}

test('unreadable read response retains its ordinary recovery category', async () => {
  globalThis.fetch = async () => new Response('not JSON', { status: 200 });
  const error = await createServices('demo').inbox.labels().catch(error => error);
  expect(error.category).toBe('invalid_response');
});

test('bus sends caller identity and returns only request data', async () => {
  const { extensionBus } = await import('../src/lib/bus.ts');
  const calls = [];
  globalThis.fetch = async (path, input) => {
    calls.push({path, ...input});
    return Response.json(path.endsWith('/request') ? {data: {draft: {body: 'Updated'}}} : {});
  };
  const bus = extensionBus('composer');
  expect(await bus.request('helper', 'v1/rewrite', {draft: {body: 'Original'}})).toEqual({draft: {body: 'Updated'}});
  await bus.emit('v1/opened', {});
  expect(calls[0].headers['x-colecto-extension']).toBe('composer');
  expect(calls[0].path).toBe('/api/v1/bus/helper/request');
  expect(JSON.parse(calls[1].body)).toEqual({name:'v1/opened',payload:{}});
});
