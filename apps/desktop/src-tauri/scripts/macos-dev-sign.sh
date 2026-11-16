#!/bin/sh
# Cargo runner (macOS). Signs the desktop binary with a *stable* identity before
# running it, so the macOS Keychain recognises every rebuild as the same app and
# never shows the "wants to use your confidential information" dialog.
#
# Identity: $PASSWORD365_SIGNING_IDENTITY, else the first "Apple Development"
# certificate in the login keychain. Without one (CI, contributors) the binary
# just runs unsigned. Only `password365-desktop` is signed; test binaries are not.
set -eu

bin="$1"
if [ "$(basename "$bin")" = "password365-desktop" ]; then
  identity="${PASSWORD365_SIGNING_IDENTITY:-$(security find-identity -v -p codesigning 2>/dev/null |
    sed -n 's/.*"\(Apple Development:[^"]*\)".*/\1/p' | head -n 1)}"
  if [ -n "$identity" ]; then
    codesign --force --sign "$identity" --identifier app.password365.desktop --timestamp=none "$bin" 2>/dev/null ||
      echo "password365: could not sign $bin with \"$identity\"; running unsigned" >&2
  else
    echo "password365: no Apple Development certificate; running unsigned (the Keychain may ask for access)" >&2
  fi
fi
exec "$@"
