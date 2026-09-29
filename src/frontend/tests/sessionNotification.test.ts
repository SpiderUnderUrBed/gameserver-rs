import { beforeEach, describe, expect, it } from 'vitest';
import { shouldNotifyOnce } from '../src/lib/utils/sessionNotification';

describe('shouldNotifyOnce', () => {
  beforeEach(() => {
    const values = new Map<string, string>();

    Object.defineProperty(globalThis, 'sessionStorage', {
      value: {
        clear: () => values.clear(),
        getItem: (name: string) => values.get(name) ?? null,
        key: (index: number) => Array.from(values.keys())[index] ?? null,
        removeItem: (name: string) => values.delete(name),
        setItem: (name: string, value: string) => values.set(name, String(value)),
        get length() {
          return values.size;
        }
      },
      configurable: true,
      writable: true
    });
  });

  it('fires once per browser session for the same key', () => {
    expect(shouldNotifyOnce('node-warning')).toBe(true);
    expect(shouldNotifyOnce('node-warning')).toBe(false);
    expect(shouldNotifyOnce('server-warning')).toBe(true);
  });
});
