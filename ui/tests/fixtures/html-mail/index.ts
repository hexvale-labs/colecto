import { readFileSync } from 'node:fs';

export const templates: {
  name: string;
  subject: string;
  sender: string;
  html: string;
  button: { name: string; href: string };
  text: string;
}[] = [
  {
    "name": "postmark-receipt",
    "subject": "Your Northstar receipt R-1042",
    "sender": "Northstar Billing <billing@northstar.test>",
    "button": {
      "name": "Download as PDF",
      "href": "https://northstar.test/receipts/R-1042.pdf"
    },
    "text": "Your annual workspace subscription is paid through November 2027."
  },
  {
    "name": "mailgun-alert",
    "subject": "Your Acme report allowance is running low",
    "sender": "Acme Reports <reports@acme.test>",
    "button": {
      "name": "Upgrade my account",
      "href": "https://acme.test/account/upgrade"
    },
    "text": "Your team has one report remaining this month."
  },
  {
    "name": "cerberus-newsletter",
    "subject": "Field Notes: November trails and stories",
    "sender": "Field Notes <newsletter@fieldnotes.test>",
    "button": {
      "name": "Read the November issue",
      "href": "https://fieldnotes.test/issues/november"
    },
    "text": "This month we follow the coastal trail from sunrise to the last ferry home."
  },
  {
    "name": "cerberus-marketing",
    "subject": "The winter collection has arrived",
    "sender": "Alder Studio <hello@alder.test>",
    "button": {
      "name": "Explore the winter collection",
      "href": "https://alder.test/collections/winter"
    },
    "text": "Discover warm layers and thoughtful essentials for the colder days ahead."
  },
  {
    "name": "responsive-action",
    "subject": "Confirm your Northstar workspace",
    "sender": "Northstar <hello@northstar.test>",
    "button": {
      "name": "Confirm your email",
      "href": "https://northstar.test/confirm/email-1042"
    },
    "text": "Confirm your email to finish setting up your Northstar workspace."
  }
].map((template) => ({
  ...template,
  html: readFileSync(new URL(`./${template.name}.html`, import.meta.url), 'utf8'),
}));
