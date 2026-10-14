import { describe, expect, it } from 'vitest';

import { shortPath } from './path';

describe('shortPath', () => {
  it('writes the home folder as a tilde', () => {
    expect(shortPath('/home/frogius/.omp/agent/mcp.json')).toBe('~/.omp/agent/mcp.json');
    expect(shortPath('/Users/me/.pi/settings.json')).toBe('~/.pi/settings.json');
    expect(shortPath('C:\\Users\\me\\.omp\\agent\\config.yml')).toBe('~/.omp/agent/config.yml');
  });

  it('keeps only the last folders of a long path', () => {
    expect(shortPath('/home/frogius/.omp/agent/some/folder/profiles/mcp.json')).toBe('…/folder/profiles/mcp.json');
    expect(shortPath('/srv/projects/app/.omp/settings.yml')).toBe('…/app/.omp/settings.yml');
  });

  it('leaves a short path outside home alone', () => {
    expect(shortPath('/etc/omp/config.yml')).toBe('/etc/omp/config.yml');
    expect(shortPath('config.yml')).toBe('config.yml');
  });
});
