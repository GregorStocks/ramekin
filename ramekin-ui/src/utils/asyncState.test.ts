import { createRoot } from "solid-js";
import { describe, expect, it } from "vitest";
import { createApiResource, createAsyncAction } from "./asyncState";

async function waitForResource() {
  await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("createAsyncAction", () => {
  it("tracks loading and returns the action result", async () => {
    await createRoot(async (dispose) => {
      const action = createAsyncAction(
        async (value: number) => value + 1,
        "Failed",
      );

      const promise = action.run(41);

      expect(action.loading()).toBe(true);
      await expect(promise).resolves.toBe(42);
      expect(action.loading()).toBe(false);
      expect(action.error()).toBeNull();

      dispose();
    });
  });

  it("extracts API error messages and clears them on the next run", async () => {
    await createRoot(async (dispose) => {
      let shouldFail = true;
      const action = createAsyncAction(async () => {
        if (shouldFail) {
          throw new Response(JSON.stringify({ error: "Server said no" }), {
            status: 400,
          });
        }
        return "ok";
      }, "Fallback message");

      await expect(action.run()).resolves.toBeUndefined();
      expect(action.loading()).toBe(false);
      expect(action.error()).toBe("Server said no");

      shouldFail = false;
      await expect(action.run()).resolves.toBe("ok");
      expect(action.error()).toBeNull();

      dispose();
    });
  });
});

describe("createApiResource", () => {
  it("loads data through a Solid resource", async () => {
    await new Promise<void>((resolve, reject) => {
      createRoot((dispose) => {
        const resource = createApiResource(async () => "loaded", "Failed");

        waitForResource()
          .then(() => {
            expect(resource.loading()).toBe(false);
            expect(resource.data()).toBe("loaded");
            expect(resource.error()).toBeNull();
            dispose();
            resolve();
          })
          .catch((err: unknown) => {
            dispose();
            reject(err);
          });
      });
    });
  });

  it("exposes extracted API error messages", async () => {
    await new Promise<void>((resolve, reject) => {
      createRoot((dispose) => {
        const resource = createApiResource(async () => {
          throw new Response(JSON.stringify({ error: "No resource" }), {
            status: 404,
          });
        }, "Failed to load");

        waitForResource()
          .then(() => {
            expect(resource.loading()).toBe(false);
            expect(resource.data()).toBeUndefined();
            expect(resource.error()).toBe("No resource");
            dispose();
            resolve();
          })
          .catch((err: unknown) => {
            dispose();
            reject(err);
          });
      });
    });
  });
});
