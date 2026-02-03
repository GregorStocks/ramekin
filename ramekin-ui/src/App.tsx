import { Show } from "solid-js";
import { Router, Route, Navigate } from "@solidjs/router";
import "./App.css";
import { AuthProvider, useAuth } from "./context/AuthContext";
import Layout from "./components/Layout";
import LoginPage from "./pages/LoginPage";
import CookbookPage from "./pages/CookbookPage";
import CreateRecipePage from "./pages/CreateRecipePage";
import ViewRecipePage from "./pages/ViewRecipePage";
import EditRecipePage from "./pages/EditRecipePage";
import CapturePage from "./pages/CapturePage";
import TagsPage from "./pages/TagsPage";
import MealPlanPage from "./pages/MealPlanPage";
import ShoppingListPage from "./pages/ShoppingListPage";
import type { ParentComponent } from "solid-js";

const ProtectedRoute: ParentComponent = (props) => {
  const { isAuthenticated } = useAuth();

  return (
    <Show when={isAuthenticated()} fallback={<Navigate href="/login" />}>
      <Layout>{props.children}</Layout>
    </Show>
  );
};

const PublicRoute: ParentComponent = (props) => {
  const { isAuthenticated } = useAuth();

  return (
    <Show when={!isAuthenticated()} fallback={<Navigate href="/" />}>
      {props.children}
    </Show>
  );
};

function App() {
  return (
    <AuthProvider>
      <Router>
        <Route
          path="/login"
          component={() => (
            <PublicRoute>
              <LoginPage />
            </PublicRoute>
          )}
        />
        <Route
          path="/"
          component={() => (
            <ProtectedRoute>
              <CookbookPage />
            </ProtectedRoute>
          )}
        />
        <Route
          path="/recipes/new"
          component={() => (
            <ProtectedRoute>
              <CreateRecipePage />
            </ProtectedRoute>
          )}
        />
        <Route
          path="/recipes/:id"
          component={() => (
            <ProtectedRoute>
              <ViewRecipePage />
            </ProtectedRoute>
          )}
        />
        <Route
          path="/recipes/:id/edit"
          component={() => (
            <ProtectedRoute>
              <EditRecipePage />
            </ProtectedRoute>
          )}
        />
        <Route
          path="/tags"
          component={() => (
            <ProtectedRoute>
              <TagsPage />
            </ProtectedRoute>
          )}
        />
        <Route
          path="/meal-plan"
          component={() => (
            <ProtectedRoute>
              <MealPlanPage />
            </ProtectedRoute>
          )}
        />
        <Route
          path="/shopping-list"
          component={() => (
            <ProtectedRoute>
              <ShoppingListPage />
            </ProtectedRoute>
          )}
        />
        <Route path="/capture" component={CapturePage} />
      </Router>
    </AuthProvider>
  );
}

export default App;
