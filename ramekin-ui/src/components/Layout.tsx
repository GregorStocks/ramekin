import { A, useSearchParams } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import type { ParentComponent } from "solid-js";
import { Show, createSignal, onCleanup, onMount } from "solid-js";

declare const __BUILD_COMMIT__: string;
declare const __BUILD_TIME__: string;

const startTime = new Date().toLocaleString();

const formatBuildTime = (iso: string) => new Date(iso).toLocaleString();

const Layout: ParentComponent = (props) => {
  const { setToken, token, authedFetch } = useAuth();
  const [searchParams] = useSearchParams();
  const [isMobileNavOpen, setIsMobileNavOpen] = createSignal(false);
  const [isUserMenuOpen, setIsUserMenuOpen] = createSignal(false);
  const [isExporting, setIsExporting] = createSignal(false);

  let userMenuRef: HTMLDivElement | undefined;
  let userMenuTriggerRef: HTMLButtonElement | undefined;

  const closeUserMenu = () => {
    setIsUserMenuOpen(false);
  };

  const closeAll = () => {
    setIsMobileNavOpen(false);
    setIsUserMenuOpen(false);
  };

  const logout = () => {
    closeAll();
    setToken(null);
  };

  const exportAllRecipes = async () => {
    if (isExporting()) return;
    setIsExporting(true);
    try {
      const t = token();
      if (!t) throw new Error("not authenticated");
      const response = await authedFetch("/api/recipes/export");
      if (!response.ok) {
        throw new Error(`export failed: ${response.status}`);
      }
      const disposition = response.headers.get("content-disposition") ?? "";
      const match = disposition.match(/filename="?([^"]+)"?/);
      const filename =
        match?.[1] ??
        `recipes-${new Date().toISOString().replace(/[:.]/g, "-")}.paprikarecipes`;
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 0);
      closeAll();
    } finally {
      setIsExporting(false);
    }
  };

  onMount(() => {
    const handlePointerDown = (event: MouseEvent) => {
      if (!isUserMenuOpen()) return;
      const target = event.target as Node | null;
      if (target && userMenuRef && userMenuRef.contains(target)) return;
      closeUserMenu();
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isUserMenuOpen()) {
        closeUserMenu();
        userMenuTriggerRef?.focus();
      }
    };
    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    onCleanup(() => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    });
  });

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
          <div class="app-nav-primary">
            <A href="/" onClick={closeAll}>
              Cookbook
            </A>
            <A href="/meal-plan" onClick={closeAll}>
              Meal Plan
            </A>
            <A href="/shopping-list" onClick={closeAll}>
              Shopping List
            </A>
          </div>
          <div class="app-nav-actions">
            <A
              href="/recipes/new"
              class="btn btn-primary btn-header"
              onClick={closeAll}
            >
              + New Recipe
            </A>
            <div
              class="user-menu"
              classList={{ open: isUserMenuOpen() }}
              ref={userMenuRef}
            >
              <button
                type="button"
                class="user-menu-trigger"
                ref={userMenuTriggerRef}
                aria-haspopup="true"
                aria-expanded={isUserMenuOpen()}
                aria-controls="user-menu-items"
                aria-label="Account menu"
                onClick={() => setIsUserMenuOpen((open) => !open)}
              >
                <svg
                  aria-hidden="true"
                  viewBox="0 0 24 24"
                  width="18"
                  height="18"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.75"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <circle cx="12" cy="8" r="4" />
                  <path d="M4 20c1.5-4 5-6 8-6s6.5 2 8 6" />
                </svg>
              </button>
              <div id="user-menu-items" class="user-menu-items">
                <A href="/tags" onClick={closeAll}>
                  Tags
                </A>
                <A href="/import" onClick={closeAll}>
                  Import
                </A>
                <button
                  type="button"
                  class="nav-link-button"
                  onClick={exportAllRecipes}
                  disabled={isExporting()}
                >
                  {isExporting() ? "Exporting…" : "Export"}
                </button>
                <div class="user-menu-divider" aria-hidden="true" />
                <button type="button" class="nav-link-button" onClick={logout}>
                  Logout
                </button>
              </div>
            </div>
          </div>
        </nav>
      </header>
      <main class="app-main">{props.children}</main>
      <Show when={searchParams.debug !== undefined}>
        <footer class="app-footer">
          Built on {__BUILD_COMMIT__} | Build time:{" "}
          {formatBuildTime(__BUILD_TIME__)} | Start time: {startTime}
        </footer>
      </Show>
    </div>
  );
};

export default Layout;
