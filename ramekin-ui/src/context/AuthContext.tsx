import {
  createContext,
  createSignal,
  createEffect,
  useContext,
} from "solid-js";
import type { ParentComponent } from "solid-js";
import { Configuration, RecipesApi, PhotosApi } from "ramekin-client";

interface AuthContextValue {
  token: () => string | null;
  setToken: (token: string | null) => void;
  isAuthenticated: () => boolean;
  getRecipesApi: () => RecipesApi;
  getPhotosApi: () => PhotosApi;
}

const AuthContext = createContext<AuthContextValue>();

export const AuthProvider: ParentComponent = (props) => {
  const [token, setTokenInternal] = createSignal<string | null>(
    localStorage.getItem("token"),
  );

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

  const value: AuthContextValue = {
    token,
    setToken,
    isAuthenticated: () => !!token(),
    getRecipesApi,
    getPhotosApi,
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
