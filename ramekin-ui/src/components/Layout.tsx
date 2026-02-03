import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import type { ParentComponent } from "solid-js";

const Layout: ParentComponent = (props) => {
  const { setToken } = useAuth();

  const logout = () => {
    setToken(null);
  };

  return (
    <div class="app-layout">
      <header class="app-header">
        <A href="/" class="app-title">
          Ramekin
        </A>
        <nav class="app-nav">
          <A href="/">My Cookbook</A>
          <A href="/meal-plan">Meal Plan</A>
          <A href="/shopping-list">Shopping List</A>
          <A href="/tags">Tags</A>
          <A href="/recipes/new" class="btn btn-primary btn-header">
            + New Recipe
          </A>
          <button onClick={logout} class="logout-button">
            Logout
          </button>
        </nav>
      </header>
      <main class="app-main">{props.children}</main>
    </div>
  );
};

export default Layout;
