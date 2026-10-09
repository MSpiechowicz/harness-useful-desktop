/**
 * WebGL2 aurora: domain-warped fbm noise in the brand colors with film-grain
 * dither and a vignette. The renderer owns the GL objects; AuroraCanvas owns
 * scheduling (rAF loop, motion setting).
 */

const VERTEX = `#version 300 es
// Full-screen triangle, no vertex buffers.
void main() {
  vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2);
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const FRAGMENT = `#version 300 es
precision highp float;
uniform vec2 uRes;
uniform float uTime;
uniform float uIntensity;
out vec4 outColor;

float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x),
             mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}

float fbm(vec2 p) {
  float sum = 0.0;
  float amp = 0.5;
  mat2 rot = mat2(1.6, 1.2, -1.2, 1.6);
  for (int i = 0; i < 4; i++) {
    sum += amp * noise(p);
    p = rot * p;
    amp *= 0.5;
  }
  return sum;
}

void main() {
  vec2 uv = (gl_FragCoord.xy * 2.0 - uRes) / min(uRes.x, uRes.y);
  float t = uTime * 0.12;

  // Stretch vertically-oriented noise into aurora curtains, then warp it twice for the silky drift.
  vec2 p = vec2(uv.x * 0.8, uv.y * 0.45);
  vec2 q = vec2(fbm(p + vec2(0.0, t)), fbm(p + vec2(5.2, 1.3) - t));
  vec2 r = vec2(fbm(p + 2.6 * q + vec2(1.7, 9.2) + t * 0.7), fbm(p + 2.6 * q + vec2(8.3, 2.8) - t * 0.6));
  float f = fbm(p + 2.4 * r);

  vec3 base = vec3(0.047, 0.058, 0.094);
  vec3 violet = vec3(0.58, 0.34, 1.0);
  vec3 indigo = vec3(0.28, 0.42, 1.0);
  vec3 cyan = vec3(0.2, 0.9, 0.95);

  vec3 glow = mix(mix(violet, indigo, q.x), cyan, smoothstep(0.35, 0.9, r.y));
  float ridge = pow(1.0 - abs(f * 2.0 - 1.0), 3.0);
  float haze = smoothstep(0.25, 0.8, f);
  float lift = 0.35 + 0.65 * smoothstep(-1.2, 0.8, uv.y);

  vec3 color = base + glow * (haze * 0.55 + ridge * 0.9) * lift * uIntensity;
  color += violet * exp(-dot(uv * vec2(0.8, 1.2), uv * vec2(0.8, 1.2)) * 1.6) * 0.1 * uIntensity;

  // Vignette and film grain (grain also hides gradient banding).
  color *= 1.0 - 0.45 * smoothstep(0.6, 1.9, length(uv));
  color += (hash(gl_FragCoord.xy + fract(uTime) * 61.7) - 0.5) * 0.03;

  outColor = vec4(color, 1.0);
}`;

/** Upper bound on rendered pixels; the aurora is smooth, so lower resolution is invisible. */
const MAX_PIXELS = 1_800_000;
const MAX_DPR = 2;

export interface AuroraRenderer {
  resize(cssWidth: number, cssHeight: number): void;
  render(seconds: number, intensity: number): void;
  dispose(): void;
}

interface Program {
  gl: WebGL2RenderingContext;
  program: WebGLProgram;
  uniforms: { res: WebGLUniformLocation | null; time: WebGLUniformLocation | null; intensity: WebGLUniformLocation | null };
}

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) {
    return null;
  }

  gl.shaderSource(shader, source);
  gl.compileShader(shader);

  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    gl.deleteShader(shader);

    return null;
  }

  return shader;
}

function link(gl: WebGL2RenderingContext): Program | null {
  const vertex = compile(gl, gl.VERTEX_SHADER, VERTEX);
  const fragment = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT);
  const program = gl.createProgram();

  if (!vertex || !fragment || !program) {
    return null;
  }

  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);

  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    return null;
  }

  return {
    gl,
    program,
    uniforms: {
      res: gl.getUniformLocation(program, 'uRes'),
      time: gl.getUniformLocation(program, 'uTime'),
      intensity: gl.getUniformLocation(program, 'uIntensity'),
    },
  };
}

/**
 * Creates the renderer, or returns null when WebGL2 is unavailable or the
 * shader fails to build (callers show the CSS fallback). `onContextChange`
 * reports loss (`true`) and restoration (`false`).
 */
export function createAuroraRenderer(canvas: HTMLCanvasElement, onContextChange: (lost: boolean) => void): AuroraRenderer | null {
  const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, powerPreference: 'low-power' });
  let program = gl ? link(gl) : null;

  if (!gl || !program) {
    return null;
  }

  const onLost = (event: Event): void => {
    event.preventDefault();
    onContextChange(true);
  };
  const onRestored = (): void => {
    program = link(gl);
    gl.viewport(0, 0, canvas.width, canvas.height);
    onContextChange(false);
  };

  canvas.addEventListener('webglcontextlost', onLost);
  canvas.addEventListener('webglcontextrestored', onRestored);

  return {
    resize(cssWidth, cssHeight) {
      const dpr = Math.min(window.devicePixelRatio || 1, MAX_DPR);
      const budget = Math.sqrt(MAX_PIXELS / Math.max(1, cssWidth * cssHeight));
      const scale = Math.min(dpr, budget);

      canvas.width = Math.max(1, Math.round(cssWidth * scale));
      canvas.height = Math.max(1, Math.round(cssHeight * scale));
      gl.viewport(0, 0, canvas.width, canvas.height);
    },

    render(seconds, intensity) {
      if (!program || gl.isContextLost()) {
        return;
      }

      gl.useProgram(program.program);
      gl.uniform2f(program.uniforms.res, canvas.width, canvas.height);
      gl.uniform1f(program.uniforms.time, seconds);
      gl.uniform1f(program.uniforms.intensity, intensity);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    },

    dispose() {
      canvas.removeEventListener('webglcontextlost', onLost);
      canvas.removeEventListener('webglcontextrestored', onRestored);

      if (program) {
        gl.deleteProgram(program.program);
      }
    },
  };
}
