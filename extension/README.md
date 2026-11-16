# Password365 browser extension (Chrome, Edge, Brave, Chromium)

The extension never stores passwords. It asks the **running Password365 desktop app**
through a small native-messaging helper (`password365-host`):

```
extension ──native messaging──▶ password365-host ──local socket──▶ Password365 app (unlocked)
```

## Try it (developer setup)

1. Build the helper and the extension:
   ```sh
   (cd ../core && cargo build --release -p password365-host)
   npm install && npm run build
   ```
2. Register the helper with your browsers (Chrome, Chrome Beta, Chromium, Brave, Edge and
   Firefox; macOS/Linux: a JSON file in each browser's NativeMessagingHosts folder; Windows: the
   JSON in `%LOCALAPPDATA%\Password365` plus a per-user registry key per browser, no admin):
   ```sh
   npm run install-host
   ```
3. Load the extension:
   - **Chrome, Edge, Brave**: open `chrome://extensions` (`edge://extensions` in Edge), enable
     **Developer mode**, click **Load unpacked** and pick `extension/dist`. The extension id must be
     `ckbcoaigdpfgiabfimlenlepjcloghae` (pinned by the `key` in the manifest).
   - **Firefox** (128+): open `about:debugging#/runtime/this-firefox`, **Load Temporary Add-on…**
     and pick `extension/dist-firefox/manifest.json` (id `extension@password365.app`). Temporary
     add-ons go away when Firefox quits; a permanent install needs the add-on signed by Mozilla.
4. Start the Password365 desktop app and unlock it.

## What it does
- **Inline menu (like 1Password)**: a Password365 icon inside login fields; focusing the field (or
  clicking the icon) opens a menu under it with the logins for the site → click to fill. On
  sign-up forms it offers **Use suggested password**.
- **Keyboard**: ↓ in a login field moves into the menu, ↑/↓ choose, Enter fills, Esc returns to the
  field; typing in the username field filters the list. **⌘⇧L / Ctrl+Shift+L** fills the site's
  login (or opens the menu if there are several) — change it at `chrome://extensions/shortcuts`.
- **Right-click › Password365 › Suggest password** on any field: generates a strong password into
  that field and its confirmation.
- **Popup**: logins saved for the current site (matched by registrable domain — lookalike
  domains never match) → **Fill**; **Generate password**.
- **Save / update prompt**: after you sign in with a login that is not in Password365, or with a
  changed password, a prompt (in a closed shadow root) offers to save / update it.

## Security rules
- Fill only on a user action (click in the popup or the inline menu); top frame only.
- The inline menu is an extension page in an iframe inside a closed shadow root: the site can't
  read the listed logins, and the background accepts menu actions only from that `menu.html`.
- The page URL always comes from the browser (`sender.url` / `tabs.get`), never from the page.
- Web pages (content scripts) cannot request fills; only the extension popup can.
- The app re-checks the website match before releasing a password.
- Pending save prompts live in `chrome.storage.session` (memory only) for at most 5 minutes.
