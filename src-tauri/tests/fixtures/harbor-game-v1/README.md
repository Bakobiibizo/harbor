# Harbor game protocol v1 fixtures

Source: `hydra-dynamix/neo-grounds` branch `harbor-games`, commit `7607d5c`.

Canonical contract: `docs/harborgame-v1.md` in that source revision.

These byte-for-byte fixtures are copied into Harbor so its Rust package reader and identity verifier can be tested independently from the TypeScript writer. Regenerate them in Neo Grounds with `pnpm fixtures:harbor-game`, then update this source revision and copy all four generated files together.

Expected file SHA-256 values:

```text
4792b0baeabbce1d106befc5d531c7ee0c1e5414e1094c892c8540015bd89bba  creator-signing-payload.json
2cc7e9c46dae71879a22cbfb583e1b2aecc091f71c5244d6f51954cee347ebf7  expected.json
96e3e3c34e5561dd511cb41d0f9626bc8d46e9e09a542af6760dd7fde0d9bad2  minimal-game.wasm
c62b6398a6177a635b0049b669a0be0bd9222230d18e12403fce6c8ba1ae71c8  valid.harborgame
```

The signing seed is the public RFC 8032 test vector embedded only in Neo Grounds fixture-generation code. It is not a production key.
