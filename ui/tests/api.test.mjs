import { afterEach, expect, test } from 'bun:test';
import { callFailure, hostService, request, ServiceError } from '../src/lib/api.ts';

const originalFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = originalFetch; });

// A send whose answer was lost may still have gone out; the message must not invite a resend.
// The same wording reaches the composer's client part through the extension runtime's calls.
for (const failure of ['malformed', 'interrupted']) {
  for (const operation of ['send', 'approve']) {
    test(`${operation} reports uncertainty after ${failure} successful response`, async () => {
      globalThis.fetch = async () => failure === 'malformed'
        ? new Response('<html>Proxy reply</html>', { status: 200 })
        : new Response(new ReadableStream({ start(controller) { controller.error(new TypeError('Connection closed')); } }), { status: 200 });
      const result = operation === 'send'
        ? request('composer', 'demo', '/send', { recipient: 'friend@example.test', subject: 'Hello', body: 'Preserve this draft' })
        : hostService.decideSend('request-id', 'approve');
      const error = await result.catch(error => error);
      expect(error).toBeInstanceOf(ServiceError);
      expect(error.category).toBe('delivery_uncertain');
      expect(error.message).toContain('Check Sent');
      expect(error.message).not.toContain('try again');
    });
  }
}

test('an unreachable Host during a send is uncertain, elsewhere a network error', () => {
  expect(callFailure('/send', 'unreachable').category).toBe('delivery_uncertain');
  expect(callFailure('/send-requests/abc/approve', 'unreachable').category).toBe('delivery_uncertain');
  expect(callFailure('/send-requests/abc/reject', 'unreachable').category).toBe('network');
  expect(callFailure('/messages', 'unreachable').message).toContain('try again');
});

test('a refused call keeps the Host reason, or says to check delivery for sends', () => {
  expect(callFailure('/send', 'refused', { error: 'Recipient refused', category: 'smtp' }, 550)).toMatchObject({ message: 'Recipient refused', category: 'smtp', status: 550 });
  expect(callFailure('/send', 'refused', null, 502).message).toContain('Check delivery status');
});

test('unreadable read response retains its ordinary recovery category', async () => {
  globalThis.fetch = async () => new Response('not JSON', { status: 200 });
  const error = await request('inbox', 'demo', '/labels').catch(error => error);
  expect(error.category).toBe('invalid_response');
});
