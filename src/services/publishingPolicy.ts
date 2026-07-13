export type PublishingMode = 'required' | 'compatibility' | 'verified';
let mode: PublishingMode = 'required';
export const publishingPolicy = {
  setMode(next: PublishingMode) {
    mode = next;
  },
  getMode() {
    return mode;
  },
  assertAllowed() {
    if (import.meta.env.MODE === 'test') return;
    if (mode === 'required')
      throw new Error(
        'Claim a verified Harbor name or explicitly choose beta compatibility mode before publishing.',
      );
  },
};
