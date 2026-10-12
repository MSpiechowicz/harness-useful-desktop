import { afterEach, describe, expect, it } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import { en } from '$lib/i18n/locales/en';
import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
import fixture from '$lib/ipc/mock/fixtures/ecosystem/status.json';

import { productDescription, productNote, productStepTitle, productTagline } from './product-copy';

const items = fixture.items as unknown as EcosystemItem[];
const messages = en as Record<string, string>;

afterEach(() => void i18n.load('en'));

describe('companion product copy', () => {
  it('has English messages that repeat the backend catalog word for word', () => {
    for (const item of items) {
      expect(messages[`ecosystem.product.${item.id}.tagline`], item.id).toBe(item.tagline);
      expect(messages[`ecosystem.product.${item.id}.description`], item.id).toBe(item.description);
      item.notes.forEach((note, index) => expect(messages[`ecosystem.product.${item.id}.note.${index}`], item.id).toBe(note));
      item.installSteps.forEach((step, index) => expect(messages[`ecosystem.product.${item.id}.step.${index}.title`], item.id).toBe(step.label));
    }
  });

  it('shows the translation in another language and the backend text for an entry it does not know', async () => {
    const [dashboard] = items;
    await i18n.load('pl');

    expect(productTagline(dashboard)).toContain('Zużycie tokenów');
    expect(productDescription(dashboard)).toContain('serwer MCP');
    expect(productNote(dashboard, 0)).toContain('`harness-dashboard mcp`');
    expect(productStepTitle('dashboard', 0, dashboard.installSteps[0].label)).toBe('Zainstaluj plik wykonywalny');

    expect(productStepTitle('dashboard', 7, 'A step added later')).toBe('A step added later');
  });
});
