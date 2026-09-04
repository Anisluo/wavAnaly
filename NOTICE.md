# NOTICE

wavAnaly is a derivative work of **Surfer**  
<https://gitlab.com/surfer-project/surfer>  
Copyright (c) the Surfer contributors.  
Licensed under the European Union Public Licence v1.2 (EUPL-1.2), see `LICENSE-EUPL-1.2.txt`.

wavAnaly is distributed under the same licence, EUPL-1.2.

Base revision: shallow clone of upstream `main`, version 0.7.0, taken on 2026-09-04.

## Modifications relative to upstream (kept up to date in CHANGELOG.md)

- Renamed the application and binary to `wavAnaly` / `wavanaly`; config directory `io.wavanaly`.
- Added multi-signal protocol decoders that produce virtual string signals
  (`libsurfer/src/decoders/`), starting with I2C (`decode_i2c` command).
- Added a Simplified Chinese user interface (`libsurfer/src/i18n/`), selectable via the
  `language` config key or the `WAVANALY_LANG` environment variable.
- Changed the default window size so the window fits inside a 1080p screen.

Internal crate names (`libsurfer`, `surfer-translation-types`, `surfer-wcp`, `surver`)
are kept unchanged to stay mergeable with upstream.

## Third-party fonts

- No CJK font is bundled. At start-up wavAnaly looks for a system CJK font (Microsoft YaHei / SimHei on Windows, Noto Sans CJK on Linux, PingFang on macOS) and adds it as a fallback for UI text.
