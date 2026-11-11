import type { Message } from './api';
export const demo: Message[] = [{ seen: false, flagged: false, id: 'demo', sender: 'Maya Chen', address: 'maya@example.test', subject: 'A small plan for the weekend', body: 'Coffee on Saturday?', date: 'Today' }];
export const worst: Message[] = [
    { seen: false, flagged: false, id: 'long', sender: 'Aleksandra Wiśniewska-Kowalczyk', address: 'bartholomew.fitzgerald@northwind-industries-holdings.example.com', subject: 'Quarterly planning — revised proposal and consolidated feedback from international teams', body: 'https://northwind-industries-holdings.example.com/workspaces/international/planning/2026/review?department=customer-experience\n\n日本語のメール本文 العربية 👩🏽‍💻', date: 'November 7, 2026, 11:59 PM' },
    { seen: false, flagged: false, id: 'emoji', sender: '👩🏽‍💻 Priya', address: 'first.last+billing-notifications@example.com', subject: '', body: '', date: '' },
    { seen: false, flagged: false, id: 'empty', sender: '', address: 'a@b.co', subject: 'Jo', body: 'One short note.', date: 'Today' },
  ];
