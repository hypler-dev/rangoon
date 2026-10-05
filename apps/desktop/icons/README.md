# Development desktop icons

Jeff explicitly selected [this exact Rangoon symbol](../../../preview/assets/brand-symbol.png) on October 5, 2026. The committed PNG is byte-for-byte identical to the supplied 1254 × 1254 image, including transparency. It also matches the application's existing brand symbol. No visual regeneration or crop was applied. OS identity is **Rangoon**, with only this logo in the icon; the splash may display **Rangoon.ai**.

The official [Tauri icon tool](https://v2.tauri.app/develop/icons/) derives native resources from that source:

```sh
npx --yes @tauri-apps/cli@2.12.1 icon preview/assets/brand-symbol.png --output /tmp/rangoon-workspace-icons
```

Copy `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns` and `icon.ico` from the temporary output to this directory. `tauri.conf.json` references these Linux/desktop PNG, macOS ICNS and Windows ICO resources. Mobile/store output is not included. These are application build resources, not signed installers or distributed releases.

| Resource | SHA-256 |
| --- | --- |
| `brand-symbol.png` | `7dfcf1dddf5a8df85dd373c147ad182d716c4b0d747868d9ced4518102fbc8e2` |
| `128x128.png` | `d7619cdd8b7ea7f1c6d9f1a923398c6e93d77672a6d949304b019caa45e380ae` |
| `128x128@2x.png` | `99de6501b91772f717c336685d83d7fe35bf32723e5f90d899217ecaa6409b8e` |
| `32x32.png` | `489a99cb5774b2a98c96966ae11f31a1b3e5ef439dfe98483fe597dfa892e4bc` |
| `icon.icns` | `cd97a388ed4006ff4c46ee8dae37857146ff5b181c5be1e905c4af4b1287f4c0` |
| `icon.ico` | `936b2731a17bbe7edf56d5590c752ac73a0f4b43dda3b6af07f77c0ca351f5f9` |
