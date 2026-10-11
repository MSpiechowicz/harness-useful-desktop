import { contrast, deltaE, oklch, oklchToHex, visionLabs, worstDeltaE, type Lab } from './color-math';
import type { LeftTone } from './limits';
import { modelMaker, type Maker } from './maker';

/**
 * The Harness Dashboard's data colors, ported from its web palette (web/src/lib/palette.ts and colors.svelte.ts) so
 * every provider, model and plan-limit bar looks the same in both apps: one reserved family per harness, model
 * shades generated around the maker's provider color, the general pool for everything else, and a neutral gray.
 */
export type Mode = 'light' | 'dark';

interface Pair {
  light: string;
  dark: string;
}

interface Family {
  light: readonly string[];
  dark: readonly string[];
}

/** Every hue family of the dashboard's palette (web/src/lib/palette.ts FAMILIES), same hex values. */
const FAMILIES = {
  orange: { light: ['#de602c'], dark: ['#f77745'] },
  blue: { light: ['#3c8cf0'], dark: ['#5da3ff'] },
  gold: { light: ['#9e8d00'], dark: ['#b6a300'] },
  plum: { light: ['#9e3db0'], dark: ['#ab40ad'] },
  teal: { light: ['#007973'], dark: ['#159085'] },
  crimson: { light: ['#bd004b'], dark: ['#e60357'] },
  indigo: { light: ['#3f00c3'], dark: ['#4923d6'] },
  lime: { light: ['#1cd135'], dark: ['#bde9bb'] },
  magenta: { light: ['#f648ca'], dark: ['#ff56d2'] },
  amber: { light: ['#532e01'], dark: ['#6f4005'] },
  cyan: { light: ['#2ac4cc'], dark: ['#464979'] },
  wine: { light: ['#5e014a'], dark: ['#9c0451'] },
  tokens: { light: ['#00a57b', '#606300', '#c086f1', '#5c3c77'], dark: ['#00be8e', '#6e7200', '#cf98ff', '#a07ebe'] },
  green: { light: ['#47a438', '#136400'], dark: ['#5ebb4f', '#85e276'] },
  sky: { light: ['#009cbc', '#005c70'], dark: ['#00b4d7', '#51dbff'] },
  violet: { light: ['#8279ee', '#4c3bab'], dark: ['#9791ff', '#c0c0ff'] },
  pink: { light: ['#d65895', '#92125a'], dark: ['#ef6fab', '#ffaacd'] },
} as const satisfies Record<string, Family>;

type FamilyName = keyof typeof FAMILIES;

/** The family each provider (harness) owns; the token types own `tokens` (web/src/lib/palette.ts RESERVED). */
const RESERVED = {
  claude: 'orange',
  codex: 'blue',
  cursor: 'gold',
  omp: 'plum',
  pi: 'teal',
  opencode: 'crimson',
  zed: 'indigo',
  cline: 'lime',
  roo: 'magenta',
  kilo: 'amber',
  gemini: 'cyan',
  copilot: 'wine',
  tokenType: 'tokens',
} as const satisfies Record<string, FamilyName>;

export type Provider = Exclude<keyof typeof RESERVED, 'tokenType'>;

const PROVIDERS = Object.keys(RESERVED).filter((name) => name !== 'tokenType') as Provider[];

function isProvider(name: string): name is Provider {
  return (PROVIDERS as string[]).includes(name);
}

/** "Other" and any provider without a color of its own. */
export const OTHER: Pair = { light: '#b5b3ab', dark: '#5f5e59' };

export function providerHex(provider: string, mode: Mode): string {
  const name = provider.toLowerCase();

  return isProvider(name) ? FAMILIES[RESERVED[name]][mode][0] : OTHER[mode];
}

/**
 * Models wear their maker's provider color (web/src/lib/palette.ts MAKER_FAMILIES): a Claude model through any
 * harness is Claude. Only these providers hand out model shades; other makers take the general pool.
 */
const MAKER_FAMILIES: Record<Maker, Provider> = { anthropic: 'claude', openai: 'codex' };

/** In the order their model shades are generated: a maker added at the end never repaints the ones before it. */
const MODEL_PROVIDERS: readonly Provider[] = Object.values(MAKER_FAMILIES);

/** The provider whose shades a model wears, or null when its maker has none (it takes the general pool). */
export function modelFamily(model: string): Provider | null {
  const maker = modelMaker(model);

  return maker ? MAKER_FAMILIES[maker] : null;
}

/** The general pool's families, in the order their colors are handed out. Each has a main color and a variant. */
const GENERAL: readonly FamilyName[] = ['green', 'sky', 'violet', 'pink'];

/** The general pool in hand-out order: every family's main color, then every family's variant. */
export function generalPool(mode: Mode): string[] {
  return [0, 1].flatMap((shade) => GENERAL.map((family) => FAMILIES[family][mode][shade]!));
}

/** The color of the `index`-th value of a dimension that owns no family; past the pool's end it is "Other". */
export function poolHex(index: number, mode: Mode): string {
  return generalPool(mode)[index] ?? OTHER[mode];
}

/** Chart surfaces the palette is checked against (the --surface token in the dashboard's app.css). */
const SURFACE: Record<Mode, string> = { light: '#fcfcfb', dark: '#1a1a19' };

/**
 * Where a provider's model shades come from: around the provider's color, in a lightness range that stays
 * readable on the surface, with chroma down to 60% of the provider's and hue within +-8 degrees.
 */
const SHADE_SPACE: Record<Mode, { l: readonly [number, number]; chroma: readonly [number, number]; hue: number }> = {
  light: { l: [0.4, 0.78], chroma: [0.6, 1], hue: 8 },
  dark: { l: [0.47, 0.8], chroma: [0.6, 1], hue: 8 },
};

/** A model shade keeps this ΔE from every general, token-type and other provider's color, and from "Other". */
const SHADE_MARGIN = 7;
const SHADE_FROM_OTHER = 9;
/** How many shades of an earlier provider are off limits to the providers after it. */
const SHADE_GUARD = 12;

interface ShadeSequence {
  picks: string[];
  rest: { hex: string; labs: Lab[]; dist: number }[];
}

const sequences = new Map<string, ShadeSequence>();

function shadeSequence(provider: Provider, mode: Mode): ShadeSequence {
  const key = `${provider}:${mode}`;
  const known = sequences.get(key);

  if (known) {
    return known;
  }

  const base = FAMILIES[RESERVED[provider]][mode][0];
  const { c, h } = oklch(base);
  const space = SHADE_SPACE[mode];

  // Everything a model shade must not be mistaken for. Earlier providers' shades count, so adding a provider
  // never repaints the ones before it.
  const taken = [
    ...GENERAL.flatMap((family) => FAMILIES[family][mode]),
    ...FAMILIES[RESERVED.tokenType][mode],
    ...PROVIDERS.filter((other) => other !== provider).map((other) => FAMILIES[RESERVED[other]][mode][0]),
    ...MODEL_PROVIDERS.slice(0, MODEL_PROVIDERS.indexOf(provider)).flatMap((earlier) => modelShades(earlier, mode, SHADE_GUARD)),
  ];

  const baseLabs = visionLabs(base);
  const rest: ShadeSequence['rest'] = [];

  for (let lightness = space.l[0]; lightness <= space.l[1] + 1e-9; lightness += 0.02) {
    for (const chromaFactor of [space.chroma[0], (space.chroma[0] + space.chroma[1]) / 2, space.chroma[1]]) {
      for (const hueShift of [-space.hue, -space.hue / 2, 0, space.hue / 2, space.hue]) {
        const hex = oklchToHex(lightness, c * chromaFactor, h + hueShift);
        const unreadable = contrast(hex, SURFACE[mode]) < 2 || oklch(hex).c < 0.075 || deltaE(hex, OTHER[mode]) < SHADE_FROM_OTHER;

        if (unreadable || hex === base || rest.some((r) => r.hex === hex) || taken.some((t) => deltaE(hex, t) < SHADE_MARGIN)) {
          continue;
        }

        const labs = visionLabs(hex);
        rest.push({ hex, labs, dist: worstDeltaE(labs, baseLabs) });
      }
    }
  }

  const sequence = { picks: [base], rest };
  sequences.set(key, sequence);

  return sequence;
}

/**
 * The n-th model shade of a provider; shade 0 is the provider's own color. Each next shade is the candidate
 * most different from all shades before it - for normal vision and both red-green deficiencies alike - so the
 * first models are far apart and later ones fill in between. Past the last candidate the shades repeat.
 * Ported from the dashboard's web/src/lib/palette.ts so the hex values match.
 */
export function modelShade(provider: Provider, mode: Mode, n: number): string {
  const sequence = shadeSequence(provider, mode);

  while (sequence.picks.length <= n && sequence.rest.length > 0) {
    let best = 0;

    for (let i = 1; i < sequence.rest.length; i++) {
      if (sequence.rest[i]!.dist > sequence.rest[best]!.dist) {
        best = i;
      }
    }

    const [pick] = sequence.rest.splice(best, 1);
    sequence.picks.push(pick!.hex);

    for (const r of sequence.rest) {
      r.dist = Math.min(r.dist, worstDeltaE(r.labs, pick!.labs));
    }
  }

  return sequence.picks[n % sequence.picks.length]!;
}

/** A provider's first `count` model shades. */
function modelShades(provider: Provider, mode: Mode, count: number): string[] {
  return Array.from({ length: count }, (_, n) => modelShade(provider, mode, n));
}

/**
 * The color of a model: models of Anthropic and OpenAI wear their provider's shades, ranked within the maker
 * (`peers` is every model in rank order, the top Anthropic model wearing Claude's own color); `null` for a model
 * of another maker, which takes the general pool instead.
 */
export function makerModelHex(model: string, peers: readonly string[], mode: Mode): string | null {
  const provider = modelFamily(model);

  if (!provider) {
    return null;
  }

  const rank = peers.filter((peer) => modelFamily(peer) === provider).indexOf(model);

  return modelShade(provider, mode, Math.max(rank, 0));
}

/** The dashboard's plan-limit bar colors: the track, the "little left" and "almost none left" fills, and their ink. */
export const LIMIT_COLORS = {
  track: { light: '#e9e8e3', dark: '#2c2c2a' },
  /** The text and tick in the warning state, and the hatching of the gap ahead of pace. */
  warnInk: { light: '#9a5800', dark: '#e0a23a' },
  badInk: { light: '#b42323', dark: '#e66767' },
  /** Fill from 10% to 25% left. */
  serious: { light: '#ec835a', dark: '#ec835a' },
  /** Fill below 10% left. */
  critical: { light: '#d03b3b', dark: '#d03b3b' },
  ink: { light: '#0b0b0b', dark: '#ffffff' },
} as const satisfies Record<string, Pair>;

/** The bar's fill: the provider's color while plenty is left, then the dashboard's serious and critical colors. */
export function limitFill(tone: LeftTone, provider: string, mode: Mode): string {
  if (tone === 'ok') {
    return providerHex(provider, mode);
  }

  return LIMIT_COLORS[tone === 'low' ? 'serious' : 'critical'][mode];
}

/** The percentage's text color: plain ink, the warning ink, or the bad ink. */
export function limitInk(tone: LeftTone, mode: Mode): string {
  if (tone === 'ok') {
    return LIMIT_COLORS.ink[mode];
  }

  return LIMIT_COLORS[tone === 'low' ? 'warnInk' : 'badInk'][mode];
}
