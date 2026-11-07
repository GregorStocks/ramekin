import { describe, expect, it, vi } from "vitest";
import vectorsJson from "../../../shared-test-vectors/auth-session.json?raw";
import {
  afterRejection,
  bearerToken,
  createAuthSession,
  isSessionRejection,
  TOKEN_STORAGE_KEY,
  type AuthState,
} from "./authSession";

const vectors = JSON.parse(vectorsJson) as {
  afterRejection: {
    name: string;
    state: AuthState;
    rejectedToken: string | null;
    expected: AuthState;
  }[];
  bearerToken: { header: string | null; expected: string | null }[];
  sessionRejection: { status: number; expected: boolean }[];
};

describe("auth-session shared vectors", () => {
  it.each(vectors.afterRejection)(
    "afterRejection: $name",
    ({ state, rejectedToken, expected }) => {
      expect(afterRejection(state, rejectedToken)).toEqual(expected);
    },
  );

  it.each(vectors.bearerToken)(
    "bearerToken($header)",
    ({ header, expected }) => {
      expect(bearerToken(header)).toBe(expected);
    },
  );

  it.each(vectors.sessionRejection)(
    "isSessionRejection($status)",
    ({ status, expected }) => {
      expect(isSessionRejection(status)).toBe(expected);
    },
  );

  it("returns the same state object when nothing changes", () => {
    const state: AuthState = { kind: "signedIn", token: "b" };
    expect(afterRejection(state, "a")).toBe(state);
  });
});

function memoryStorage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));
  return {
    values,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    removeItem: (key: string) => void values.delete(key),
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (err: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const response = (status: number) => new Response(null, { status });

describe("createAuthSession", () => {
  it("restores a stored token and persists sign-in and sign-out", () => {
    const storage = memoryStorage({ [TOKEN_STORAGE_KEY]: "stored" });
    const session = createAuthSession({
      storage,
      fetch: vi.fn(),
      listTags: async () => [],
    });
    expect(session.authState()).toEqual({ kind: "signedIn", token: "stored" });

    session.signIn("fresh");
    expect(storage.values.get(TOKEN_STORAGE_KEY)).toBe("fresh");
    session.signOut();
    expect(session.authState()).toEqual({ kind: "signedOut" });
    expect(storage.values.has(TOKEN_STORAGE_KEY)).toBe(false);
  });

  it("expires the session on a 401 for its own token", async () => {
    const storage = memoryStorage({ [TOKEN_STORAGE_KEY]: "a" });
    const fetchMock = vi.fn(async () => response(401));
    const session = createAuthSession({
      storage,
      fetch: fetchMock,
      listTags: async () => [],
    });

    await session.authedFetch("/api/recipes");

    const headers = new Headers(
      (fetchMock.mock.calls[0] as unknown as [string, RequestInit])[1].headers,
    );
    expect(headers.get("Authorization")).toBe("Bearer a");
    expect(session.authState()).toEqual({ kind: "expired" });
    expect(session.token()).toBeNull();
    expect(storage.values.has(TOKEN_STORAGE_KEY)).toBe(false);
  });

  it("keeps a new session when a request from the old one comes back 401", async () => {
    const pending = deferred<Response>();
    const session = createAuthSession({
      storage: memoryStorage({ [TOKEN_STORAGE_KEY]: "old" }),
      fetch: () => pending.promise,
      listTags: async () => [],
    });

    const request = session.authedFetch("/api/recipes");
    session.signIn("new");
    pending.resolve(response(401));
    await request;

    expect(session.authState()).toEqual({ kind: "signedIn", token: "new" });
  });

  it("uses the token in an explicit Authorization header", async () => {
    const session = createAuthSession({
      storage: memoryStorage({ [TOKEN_STORAGE_KEY]: "current" }),
      fetch: async () => response(401),
      listTags: async () => [],
    });

    await session.authedFetch("/api/recipes", {
      headers: { Authorization: "Bearer someone-else" },
    });

    expect(session.authState()).toEqual({
      kind: "signedIn",
      token: "current",
    });
  });

  it("ignores a tag response for a session that has since changed", async () => {
    const first = deferred<string[]>();
    const second = deferred<string[]>();
    const listTags = vi
      .fn<() => Promise<string[]>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const session = createAuthSession({
      storage: memoryStorage(),
      fetch: vi.fn(),
      listTags,
    });

    session.signIn("a");
    session.signIn("b");
    expect(session.tagsLoading()).toBe(true);

    second.resolve(["b-tag"]);
    await Promise.resolve();
    await Promise.resolve();
    expect(session.tags()).toEqual(["b-tag"]);
    expect(session.tagsLoading()).toBe(false);

    first.resolve(["a-tag"]);
    await Promise.resolve();
    await Promise.resolve();
    expect(session.tags()).toEqual(["b-tag"]);
    expect(session.tagsLoading()).toBe(false);
  });

  it("keeps tagsLoading on until the latest overlapping refresh ends", async () => {
    const first = deferred<string[]>();
    const second = deferred<string[]>();
    const listTags = vi
      .fn<() => Promise<string[]>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const session = createAuthSession({
      storage: memoryStorage({ [TOKEN_STORAGE_KEY]: "a" }),
      fetch: vi.fn(),
      listTags,
    });

    const one = session.refreshTags();
    const two = session.refreshTags();
    first.resolve(["stale"]);
    await one;
    expect(session.tagsLoading()).toBe(true);
    expect(session.tags()).toEqual([]);

    second.resolve(["fresh"]);
    await two;
    expect(session.tagsLoading()).toBe(false);
    expect(session.tags()).toEqual(["fresh"]);
  });

  it("clears tags on sign-out and drops the in-flight response", async () => {
    const pending = deferred<string[]>();
    const session = createAuthSession({
      storage: memoryStorage({ [TOKEN_STORAGE_KEY]: "a" }),
      fetch: vi.fn(),
      listTags: () => pending.promise,
    });

    const refresh = session.refreshTags();
    session.signOut();
    pending.resolve(["a-tag"]);
    await refresh;

    expect(session.tags()).toEqual([]);
    expect(session.tagsLoading()).toBe(false);
  });
});
