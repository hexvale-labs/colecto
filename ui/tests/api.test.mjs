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
