/**
 * The optional bridge an injector can expose to Corrode. Everything is optional: Corrode
 * runs fine in a plain browser tab with no bridge at all.
 *
 * An Electron injector would typically expose it from its preload script with
 * `contextBridge.exposeInMainWorld("CorrodeNative", { ... })`.
 * See docs/INJECTOR.md for the full contract.
 */
export interface CorrodeNative {
  /** Name of the injector/client, shown in the About tab. */
  name?: string;
  /** Version of the injector/client. */
  version?: string;
  /**
   * Synchronous key/value storage that survives Discord clearing its web storage
   * (e.g. backed by a JSON file in the injector's data directory).
   */
  storage?: {
    get(key: string): string | null | undefined;
    set(key: string, value: string): void;
    delete(key: string): void;
    keys?(): string[];
  };
  /** Fetch text without CORS/CSP restrictions (for remote plugins and themes). */
  fetchText?(url: string): Promise<string>;
  /** Restart the Discord client. */
  relaunch?(): void;
  /** Open the renderer's developer tools. */
  openDevTools?(): void;
  /** Remove Corrode from the client (the injector's ejector). Corrode unloads itself first. */
  eject?(): void | Promise<void>;
}

export const NATIVE_GLOBAL = "CorrodeNative";

export function getNative(): CorrodeNative {
  const n = (globalThis as any)[NATIVE_GLOBAL];
  return n && typeof n === "object" ? n : {};
}
