import { createContext, useContext } from "solid-js";
import type { Accessor, ParentComponent } from "solid-js";
import {
  Configuration,
  RecipesApi,
  PhotosApi,
  ScrapeApi,
  EditRecipeApi,
  NormalizeApi,
  TagsApi,
  MealPlansApi,
  ShoppingListApi,
  SyncApi,
  ImportApi,
  UsersApi,
  ClientLogsApi,
  IngredientNamesApi,
} from "ramekin-client";
import { createAuthSession, type AuthState } from "./authSession";

interface AuthContextValue {
  authState: Accessor<AuthState>;
  token: () => string | null;
  signIn: (token: string) => void;
  signOut: () => void;
  isAuthenticated: () => boolean;
  /** True when the last session ended because the server rejected its token. */
  sessionExpired: () => boolean;
  getRecipesApi: () => RecipesApi;
  getPhotosApi: () => PhotosApi;
  getScrapeApi: () => ScrapeApi;
  getEditRecipeApi: () => EditRecipeApi;
  getNormalizeApi: () => NormalizeApi;
  getTagsApi: () => TagsApi;
  getMealPlansApi: () => MealPlansApi;
  getShoppingListApi: () => ShoppingListApi;
  getSyncApi: () => SyncApi;
  getImportApi: () => ImportApi;
  getUsersApi: () => UsersApi;
  getClientLogsApi: () => ClientLogsApi;
  getIngredientNamesApi: () => IngredientNamesApi;
  // Cached tags - fetched once per session, shared across components
  tags: Accessor<string[]>;
  tagsLoading: Accessor<boolean>;
  refreshTags: () => Promise<void>;
  authedFetch: (
    input: RequestInfo | URL,
    init?: RequestInit,
  ) => Promise<Response>;
}

const AuthContext = createContext<AuthContextValue>();

export const AuthProvider: ParentComponent = (props) => {
  const getAuthedConfig = () =>
    new Configuration({
      basePath: "",
      accessToken: () => session.token() ?? "",
      fetchApi: (input, init) => session.authedFetch(input, init),
    });

  const getTagsApi = () => new TagsApi(getAuthedConfig());

  const session = createAuthSession({
    storage: localStorage,
    fetch: (input, init) => fetch(input, init),
    listTags: async () =>
      (await getTagsApi().listAllTags()).tags.map((t) => t.name),
  });
  void session.refreshTags();

  const value: AuthContextValue = {
    authState: session.authState,
    token: session.token,
    signIn: session.signIn,
    signOut: session.signOut,
    isAuthenticated: () => session.authState().kind === "signedIn",
    sessionExpired: () => session.authState().kind === "expired",
    getRecipesApi: () => new RecipesApi(getAuthedConfig()),
    getPhotosApi: () => new PhotosApi(getAuthedConfig()),
    getScrapeApi: () => new ScrapeApi(getAuthedConfig()),
    getEditRecipeApi: () => new EditRecipeApi(getAuthedConfig()),
    getNormalizeApi: () => new NormalizeApi(getAuthedConfig()),
    getTagsApi,
    getMealPlansApi: () => new MealPlansApi(getAuthedConfig()),
    getShoppingListApi: () => new ShoppingListApi(getAuthedConfig()),
    getSyncApi: () => new SyncApi(getAuthedConfig()),
    getImportApi: () => new ImportApi(getAuthedConfig()),
    getUsersApi: () => new UsersApi(getAuthedConfig()),
    getClientLogsApi: () => new ClientLogsApi(getAuthedConfig()),
    getIngredientNamesApi: () => new IngredientNamesApi(getAuthedConfig()),
    tags: session.tags,
    tagsLoading: session.tagsLoading,
    refreshTags: session.refreshTags,
    authedFetch: session.authedFetch,
  };

  return (
    <AuthContext.Provider value={value}>{props.children}</AuthContext.Provider>
  );
};

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
};
