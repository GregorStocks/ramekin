import { createSignal, type Accessor } from "solid-js";

/**
 * The sign-in state. A session ends as `expired` when the server rejects its
 * token, so the login page can say why the user is there.
 */
export type AuthState =
  | { kind: "signedOut" }
  | { kind: "signedIn"; token: string }
  | { kind: "expired" };

export const TOKEN_STORAGE_KEY = "token";

export function tokenOf(state: AuthState): string | null {
  return state.kind === "signedIn" ? state.token : null;
}

/**
 * Only 401 means the session is gone. A 403 is a refusal that signing in
 * again would not fix.
 */
export function isSessionRejection(status: number): boolean {
  return status === 401;
}

/**
 * The state after the server rejected (401) a request sent with
 * `rejectedToken`. A late 401 for an earlier session's token, or for a
 * request sent without one, leaves the current session alone.
 *
 * This, `isSessionRejection` and `bearerToken` mirror
 * ramekin-ios/Shared/AuthSessionSupport.swift and are pinned by
 * shared-test-vectors/auth-session.json.
 */
export function afterRejection(
  state: AuthState,
  rejectedToken: string | null,
): AuthState {
  if (state.kind === "signedIn" && state.token === rejectedToken) {
    return { kind: "expired" };
  }
  return state;
}

/** The token in an `Authorization: Bearer <token>` header value. */
export function bearerToken(header: string | null): string | null {
  const prefix = "Bearer ";
  if (!header?.startsWith(prefix)) return null;
  const token = header.slice(prefix.length);
  return token === "" ? null : token;
}

interface TokenStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

interface AuthSessionOptions {
  storage: TokenStorage;
  fetch: typeof fetch;
  /** Load the tag list. Called with the session's `authedFetch` in place. */
  listTags: () => Promise<string[]>;
}

export interface AuthSession {
  authState: Accessor<AuthState>;
  token: Accessor<string | null>;
  signIn: (token: string) => void;
  signOut: () => void;
  authedFetch: (
    input: RequestInfo | URL,
    init?: RequestInit,
  ) => Promise<Response>;
  tags: Accessor<string[]>;
  tagsLoading: Accessor<boolean>;
  refreshTags: () => Promise<void>;
}

/**
 * Owns the web client's session: the persisted token, 401 handling that only
 * expires the token a request was sent with, and the cached tag list, whose
 * responses only land if they're for the latest request of the current
 * session. Tags load on every sign-in; for a session restored from storage,
 * the caller calls `refreshTags()` once `listTags` can run.
 */
export function createAuthSession(options: AuthSessionOptions): AuthSession {
  const stored = options.storage.getItem(TOKEN_STORAGE_KEY);
  const [authState, setAuthStateSignal] = createSignal<AuthState>(
    stored ? { kind: "signedIn", token: stored } : { kind: "signedOut" },
  );
  const token = () => tokenOf(authState());

  const [tags, setTags] = createSignal<string[]>([]);
  const [tagsLoading, setTagsLoading] = createSignal(false);
  let tagsRequestId = 0;

  const refreshTags = async () => {
    const requestId = ++tagsRequestId;
    const requestToken = token();
    if (!requestToken) {
      setTags([]);
      setTagsLoading(false);
      return;
    }
    setTagsLoading(true);
    try {
      const names = await options.listTags();
      if (requestId === tagsRequestId && token() === requestToken) {
        setTags(names);
      }
    } catch {
      // Ignore errors loading tags
    } finally {
      if (requestId === tagsRequestId) {
        setTagsLoading(false);
      }
    }
  };

  const setAuthState = (next: AuthState) => {
    const previousToken = token();
    setAuthStateSignal(next);
    const nextToken = tokenOf(next);
    if (nextToken) {
      options.storage.setItem(TOKEN_STORAGE_KEY, nextToken);
    } else {
      options.storage.removeItem(TOKEN_STORAGE_KEY);
    }
    if (nextToken !== previousToken) {
      void refreshTags();
    }
  };

  const authedFetch = async (
    input: RequestInfo | URL,
    init?: RequestInit,
  ): Promise<Response> => {
    const headers = new Headers(init?.headers);
    const current = token();
    if (current && !headers.has("Authorization")) {
      headers.set("Authorization", `Bearer ${current}`);
    }
    const sentToken = bearerToken(headers.get("Authorization"));

    const response = await options.fetch(input, { ...init, headers });
    if (isSessionRejection(response.status)) {
      const next = afterRejection(authState(), sentToken);
      if (next !== authState()) setAuthState(next);
    }
    return response;
  };

  return {
    authState,
    token,
    signIn: (newToken) => setAuthState({ kind: "signedIn", token: newToken }),
    signOut: () => setAuthState({ kind: "signedOut" }),
    authedFetch,
    tags,
    tagsLoading,
    refreshTags,
  };
}
