import {
  createContext,
  createSignal,
  createEffect,
  useContext,
} from "solid-js";
import type { Accessor, ParentComponent } from "solid-js";
import {
  Configuration,
  RecipesApi,
  PhotosApi,
  ScrapeApi,
  EnrichApi,
  TagsApi,
  MealPlansApi,
} from "ramekin-client";

interface AuthContextValue {
  token: () => string | null;
  setToken: (token: string | null) => void;
  isAuthenticated: () => boolean;
  getRecipesApi: () => RecipesApi;
  getPhotosApi: () => PhotosApi;
  getScrapeApi: () => ScrapeApi;
  getEnrichApi: () => EnrichApi;
  getTagsApi: () => TagsApi;
  getMealPlansApi: () => MealPlansApi;
  // Cached tags - fetched once, shared across components
  tags: Accessor<string[]>;
  tagsLoading: Accessor<boolean>;
  refreshTags: () => Promise<void>;
}

const AuthContext = createContext<AuthContextValue>();

export const AuthProvider: ParentComponent = (props) => {
  const [token, setTokenInternal] = createSignal<string | null>(
    localStorage.getItem("token"),
  );
  const [tags, setTags] = createSignal<string[]>([]);
  const [tagsLoading, setTagsLoading] = createSignal(false);

  const setToken = (newToken: string | null) => {
    setTokenInternal(newToken);
  };

  createEffect(() => {
    const t = token();
    if (t) {
      localStorage.setItem("token", t);
    } else {
      localStorage.removeItem("token");
    }
  });

  const getAuthedConfig = () =>
    new Configuration({
      basePath: "",
      accessToken: () => token() ?? "",
    });

  const getRecipesApi = () => new RecipesApi(getAuthedConfig());
  const getPhotosApi = () => new PhotosApi(getAuthedConfig());
  const getScrapeApi = () => new ScrapeApi(getAuthedConfig());
  const getEnrichApi = () => new EnrichApi(getAuthedConfig());
  const getTagsApi = () => new TagsApi(getAuthedConfig());
  const getMealPlansApi = () => new MealPlansApi(getAuthedConfig());

  const refreshTags = async () => {
    if (!token()) {
      setTags([]);
      return;
    }
    setTagsLoading(true);
    try {
      const response = await getTagsApi().listAllTags();
      setTags(response.tags.map((t) => t.name));
    } catch {
      // Ignore errors loading tags
    } finally {
      setTagsLoading(false);
    }
  };

  // Fetch tags when token changes
  createEffect(() => {
    if (token()) {
      refreshTags();
    } else {
      setTags([]);
    }
  });

  const value: AuthContextValue = {
    token,
    setToken,
    isAuthenticated: () => !!token(),
    getRecipesApi,
    getPhotosApi,
    getScrapeApi,
    getEnrichApi,
    getTagsApi,
    getMealPlansApi,
    tags,
    tagsLoading,
    refreshTags,
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
