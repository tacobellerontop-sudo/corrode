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
  /**
   * Update checks, for clients that can update themselves and/or the Corrode build they load.
   * When present, Corrode shows an "Updates" page and checks once at startup.
   */
  updates?: {
    check(): Promise<UpdateInfo>;
    /** Download the latest Corrode build. It takes effect after `relaunch()`. */
    updateCorrode?(): Promise<void>;
    /** Download and start the client's own update (or open its download page). */
    updateClient?(): Promise<void>;
  };
  /**
   * A themes folder on disk. When present, `.css` files in it show up on the Themes page and the
   * theme creator saves there. Paths are bare file names; media paths look like `media/<name>`.
   */
  themes?: {
    list(): Promise<{ dir?: string; files: ThemeFileInfo[] }>;
    read(file: string): Promise<string>;
    write(file: string, css: string): Promise<void>;
    delete(file: string): Promise<void>;
    openFolder?(): Promise<void>;
    /** Saves a background picture or video into the folder; resolves to its path (`media/<name>`). */
    writeMedia?(name: string, data: Blob | ArrayBuffer | Uint8Array): Promise<string>;
    /** Reads a file saved with `writeMedia`. */
    readMedia?(path: string): Promise<ArrayBuffer>;
  };
}

export interface ThemeFileInfo {
  file: string;
  /** Last modified, in ms since 1970. A change makes Corrode read the file again. */
  modified: number;
  size?: number;
}

export interface UpdateTarget {
  /** What is running now. */
  version?: string;
  commit?: string;
  /** What is published. */
  latestVersion?: string;
  latestCommit?: string;
  available: boolean;
  /** Set when the check for this target failed. */
  error?: string;
}

export interface UpdateInfo {
  client: UpdateTarget;
  corrode: UpdateTarget;
}

export const NATIVE_GLOBAL = "CorrodeNative";

export function getNative(): CorrodeNative {
  const n = (globalThis as any)[NATIVE_GLOBAL];
  return n && typeof n === "object" ? n : {};
}
