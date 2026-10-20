# DeepL Popup

A Windows tray app that translates selected text in a popup. Select text in any app, press **Ctrl+F9**, and the translation appears at the cursor. It is modeled on the DeepL desktop app's quick translate popup and uses the [DeepL API](https://www.deepl.com/pro-api).

> Unofficial project, not affiliated with or endorsed by DeepL SE. "DeepL" is a trademark of DeepL SE.

## Usage

You need Windows 11 and a DeepL API key (Free or Pro). Enter the key in Settings from the tray menu.

The interface is in English or Korean. It follows the Windows display language; you can pick one in Settings.

| Key | Action |
|---|---|
| Ctrl+F9 | Translate the selected text |
| Space | Choose the target language |
| Ctrl+C | Copy the translation and close |
| Enter, Ctrl+R | Replace the selection with the translation |
| Esc | Close |

The API key is stored in Windows Credential Manager, other settings in `%APPDATA%\DeepL Popup\settings.json`. Selected text is sent to the DeepL API.

## Build

```powershell
cargo build --release
```

UI text is written in English in `ui/app.slint` with `@tr(...)`; the Korean translation is in `lang/ko/LC_MESSAGES/deepl-popup.po` and is built into the executable. Messages produced in Rust are in `src/messages.rs`.

`tools\generate-icons.ps1` regenerates the icons and needs the [Galmuri14](https://github.com/quiple/galmuri) font installed.

## License

[MIT](LICENSE).

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

The UI uses [Slint](https://slint.dev) under the [Slint Royalty-free License 2.0](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md). The icon glyph is drawn with [Galmuri](https://github.com/quiple/galmuri) (SIL Open Font License 1.1).
