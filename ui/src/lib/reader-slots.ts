import type { Message, MessageContent } from './api';

/** What the reader hands to the toolbar and body slots the mail shell fills with other
 *  bundled extensions' components (see docs/extensions/README.md, "Reader slots"). The
 *  reader never imports those extensions. */
export interface ReaderSlot {
  id: string;
  message: Message;
  /** Null while the body is still loading or when it failed. */
  content: MessageContent | null;
  /** Open the normal reply composer with `body` above the quoted message. Absent when the
   *  composer is off. */
  reply?: (body: string) => void;
}
