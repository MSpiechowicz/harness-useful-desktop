/**
 * Color math behind the model shades, ported from the Harness Dashboard (web/src/lib/colorMath.ts) so the same
 * inputs give the same hex values: OKLab / OKLCH, colour-vision-deficiency simulation and WCAG contrast.
 * Distances (ΔE) are Euclidean in OKLab x100. CVD is simulated with Machado, Oliveira & Fernandes (2009)
 * at severity 1.0.
 */
type Cvd = 'protan' | 'deutan';
type Rgb = [number, number, number];
export type Lab = [number, number, number];

const MACHADO: Record<Cvd, number[][]> = {
  protan: [
    [0.152286, 1.052583, -0.204868],
    [0.114503, 0.786281, 0.099216],
    [-0.003882, -0.048116, 1.051998],
  ],
  deutan: [
    [0.367322, 0.860646, -0.227968],
    [0.280085, 0.672501, 0.047413],
    [-0.01182, 0.04294, 0.968881],
  ],
};

const HEX = /^#[0-9a-f]{6}$/i;

function srgb(hex: string): Rgb {
  if (!HEX.test(hex)) {
    throw new Error(`not a #rrggbb color: ${hex}`);
  }

  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255) as Rgb;
}

const toLinear = (c: number): number => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);

function linear(hex: string): Rgb {
  return srgb(hex).map(toLinear) as Rgb;
}

function labFromLinear([r, g, b]: Rgb): Lab {
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);

  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function simulate(hex: string, kind: Cvd): Rgb {
  const [r, g, b] = linear(hex);
  const clamp = (c: number): number => Math.max(0, Math.min(1, c));

  return MACHADO[kind].map((row) => clamp(row[0]! * r + row[1]! * g + row[2]! * b)) as Rgb;
}

/** OKLCH lightness, chroma and hue (degrees). */
export function oklch(hex: string): { l: number; c: number; h: number } {
  const [l, a, b] = labFromLinear(linear(hex));

  return { l, c: Math.hypot(a, b), h: ((Math.atan2(b, a) * 180) / Math.PI + 360) % 360 };
}

function linearFromOklch(l: number, c: number, h: number): Rgb {
  const a = c * Math.cos((h * Math.PI) / 180);
  const b = c * Math.sin((h * Math.PI) / 180);
  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;

  return [
    4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
    -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
    -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_,
  ];
}

const inGamut = (rgb: number[]): boolean => rgb.every((v) => v >= -1e-4 && v <= 1 + 1e-4);

/** OKLCH to #rrggbb. Chroma is lowered until the color fits inside sRGB; lightness and hue are kept. */
export function oklchToHex(l: number, c: number, h: number): string {
  let rgb = linearFromOklch(l, c, h);

  if (!inGamut(rgb)) {
    let lo = 0;
    let hi = c;

    for (let i = 0; i < 30; i++) {
      const mid = (lo + hi) / 2;

      if (inGamut(linearFromOklch(l, mid, h))) {
        lo = mid;
      } else {
        hi = mid;
      }
    }

    rgb = linearFromOklch(l, lo, h);
  }

  const toSrgb = (v: number): number => {
    const x = Math.max(0, Math.min(1, v));

    return x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055;
  };

  return `#${rgb.map((v) => Math.round(toSrgb(v) * 255).toString(16).padStart(2, '0')).join('')}`;
}

/** OKLab ΔE x100 between two colors as seen with normal vision. */
export function deltaE(a: string, b: string): number {
  const x = labFromLinear(linear(a));
  const y = labFromLinear(linear(b));

  return 100 * Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]);
}

/** OKLab coordinates as seen with normal vision, protanopia and deuteranopia, for repeated comparisons. */
export function visionLabs(hex: string): Lab[] {
  return [labFromLinear(linear(hex)), labFromLinear(simulate(hex, 'protan')), labFromLinear(simulate(hex, 'deutan'))];
}

/** ΔE x100 for whichever of normal vision, protanopia and deuteranopia tells the two colors apart least. */
export function worstDeltaE(a: Lab[], b: Lab[]): number {
  return 100 * Math.min(...a.map((x, i) => Math.hypot(x[0] - b[i]![0], x[1] - b[i]![1], x[2] - b[i]![2])));
}

function luminance(hex: string): number {
  const [r, g, b] = linear(hex);

  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];

  return (hi + 0.05) / (lo + 0.05);
}
