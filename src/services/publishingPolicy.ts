export type PublishingMode = 'required' | 'unverified' | 'verified';
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
        'Claim a verified Harbor name or explicitly publish as an unverified identity.',
      );
  },
};
