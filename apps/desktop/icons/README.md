# Development Windows icon

`icon.ico` is a build resource derived from the existing [Rangoon brand symbol](../../../preview/assets/brand-symbol.png), using the official [Tauri icon tool](https://v2.tauri.app/develop/icons/). It fixes Windows executable resource compilation; it is not an installer or release artifact.

Generation command, run from the repository root:

```sh
npx --yes @tauri-apps/cli@2.12.1 icon preview/assets/brand-symbol.png --output /tmp/rangoon-native-icons
```

Only `icon.ico` was copied from that temporary output directory. No mobile or store artifacts are included.

- Source SHA-256: `7dfcf1dddf5a8df85dd373c147ad182d716c4b0d747868d9ced4518102fbc8e2`
- ICO SHA-256: `936b2731a17bbe7edf56d5590c752ac73a0f4b43dda3b6af07f77c0ca351f5f9`
