type ProfileRuntimeReset = () => void;

const runtimeResets = new Set<ProfileRuntimeReset>();

/** Register an app-lifetime resource that must drop profile-bound work on lock or switch. */
export function registerProfileRuntimeReset(reset: ProfileRuntimeReset): () => void {
  runtimeResets.add(reset);
  return () => runtimeResets.delete(reset);
}

export function resetRegisteredProfileResources(): void {
  for (const reset of [...runtimeResets]) {
    try {
      reset();
    } catch (error) {
      console.warn('[ProfileRuntime] Failed to reset a registered resource', error);
    }
  }
}
