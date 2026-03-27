import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import type { ParentComponent } from "solid-js";
import { createSignal } from "solid-js";

declare const __BUILD_COMMIT__: string;
declare const __BUILD_TIME__: string;

const startTime = new Date().toLocaleString();

const formatBuildTime = (iso: string) => new Date(iso).toLocaleString();

const Layout: ParentComponent = (props) => {
  const { setToken } = useAuth();
  const [isMobileNavOpen, setIsMobileNavOpen] = createSignal(false);

  const closeMobileNav = () => {
    setIsMobileNavOpen(false);
  };

  const logout = () => {
    closeMobileNav();
    setToken(null);
  };

  return (
    <div class="app-layout">
      <header class="app-header">
        <A href="/" class="app-title">
          Ramekin
        </A>
        <button
          type="button"
          class="mobile-nav-toggle"
          aria-label={
            isMobileNavOpen() ? "Close navigation menu" : "Open navigation menu"
          }
          aria-controls="app-navigation"
          aria-expanded={isMobileNavOpen()}
          onClick={() => setIsMobileNavOpen((open) => !open)}
        >
          <span aria-hidden="true">{isMobileNavOpen() ? "Close" : "Menu"}</span>
        </button>
        <nav
          id="app-navigation"
          classList={{
            "app-nav": true,
            "app-nav-open": isMobileNavOpen(),
          }}
        >
          <A href="/" onClick={closeMobileNav}>
            My Cookbook
          </A>
          <A href="/meal-plan" onClick={closeMobileNav}>
            Meal Plan
          </A>
          <A href="/shopping-list" onClick={closeMobileNav}>
            Shopping List
          </A>
          <A href="/tags" onClick={closeMobileNav}>
            Tags
          </A>
          <A
            href="/recipes/new"
            class="btn btn-primary btn-header"
            onClick={closeMobileNav}
          >
            + New Recipe
          </A>
          <button onClick={logout} class="logout-button">
            Logout
          </button>
        </nav>
      </header>
      <main class="app-main">{props.children}</main>
      <footer class="app-footer">
        Built on {__BUILD_COMMIT__} | Build time:{" "}
        {formatBuildTime(__BUILD_TIME__)} | Start time: {startTime}
      </footer>
    </div>
  );
};

export default Layout;
