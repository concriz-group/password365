"""Release helpers for the desktop app (used by .github/workflows/release.yml).

    python3 latest_json.py manifest <version> <bundles dir> <download url> <CHANGELOG.md> > latest.json
    python3 latest_json.py notes <version> <CHANGELOG.md> > notes.md

`manifest` builds the updater's `latest.json` (read by the app from the newest
GitHub release) from the signed update bundles in <bundles dir>, each next to
its `.sig`; <download url> is where that release's files are, e.g.
https://github.com/concriz-group/password365/releases/download/v0.2.0.
`notes` prints what changed in <version>, from CHANGELOG.md (its "## [0.2.0]"
section): the release page and the app's update notice show it.
"""
import datetime
import json
import os
import re
import sys

# Bundle name ending → the updater's platform keys it serves.
PLATFORMS = [
    (".app.tar.gz", ["darwin-aarch64", "darwin-x86_64"]),
    ("-setup.exe", ["windows-x86_64"]),
    (".AppImage", ["linux-x86_64"]),
]


def notes(changelog, version):
    """The body of CHANGELOG.md's section for `version` ("" if there is none)."""
    match = re.search(
        rf"^## \[?v?{re.escape(version)}\]?[^\n]*\n(.*?)(?=^## |\Z)", changelog, re.M | re.S
    )
    return match.group(1).strip() if match else ""


def manifest(version, files, signature_of, download_url, notes_text="", now=None):
    """`files`: bundle file names; `signature_of(name)`: its `.sig` text."""
    platforms = {}
    for suffix, keys in PLATFORMS:
        matches = [f for f in files if f.endswith(suffix)]
        if len(matches) > 1:
            raise ValueError(f"more than one {suffix} bundle: {matches}")
        if not matches:
            continue
        name = matches[0]
        entry = {"signature": signature_of(name).strip(), "url": f"{download_url.rstrip('/')}/{name}"}
        for key in keys:
            platforms[key] = entry
    if not platforms:
        raise ValueError("no update bundles found")
    stamp = (now or datetime.datetime.now(datetime.timezone.utc)).strftime("%Y-%m-%dT%H:%M:%SZ")
    return {"version": version, "notes": notes_text, "pub_date": stamp, "platforms": platforms}


def main(argv):
    command, version = argv[1], argv[2].removeprefix("v")
    if command == "notes":
        print(notes(open(argv[3], encoding="utf-8").read(), version))
        return
    folder, download_url, changelog = argv[3:6]
    files = sorted(os.listdir(folder))
    read = lambda name: open(os.path.join(folder, name + ".sig"), encoding="utf-8").read()
    text = notes(open(changelog, encoding="utf-8").read(), version)
    print(json.dumps(manifest(version, files, read, download_url, text), indent=2))


if __name__ == "__main__":
    main(sys.argv)
