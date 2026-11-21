import datetime
import unittest

from latest_json import manifest, notes

NOW = datetime.datetime(2026, 11, 16, 12, 0, tzinfo=datetime.timezone.utc)
SIGS = {"Password365-macOS.app.tar.gz": "mac-sig\n", "Password365-Windows-setup.exe": "win-sig", "Password365-Linux.AppImage": "linux-sig"}
URL = "https://github.com/concriz-group/password365/releases/download/v0.2.0"
CHANGELOG = """# Changelog

## [Unreleased]
- Something not out yet.

## [0.2.0] — 2026-12-01
### Added
- Skip a version.

## [0.1.0] — 2026-11-16
- First release.
"""


class Manifest(unittest.TestCase):
    def test_every_platform_points_at_its_signed_bundle(self):
        files = list(SIGS) + [f + ".sig" for f in SIGS] + ["Password365-Linux.deb"]
        m = manifest("0.2.0", files, SIGS.__getitem__, URL + "/", "Skip a version.", NOW)
        self.assertEqual((m["version"], m["notes"]), ("0.2.0", "Skip a version."))
        self.assertEqual(m["pub_date"], "2026-11-16T12:00:00Z")
        mac = m["platforms"]["darwin-aarch64"]
        self.assertEqual(mac, m["platforms"]["darwin-x86_64"], "one universal bundle for both Macs")
        self.assertEqual(mac["signature"], "mac-sig")
        self.assertEqual(mac["url"], URL + "/Password365-macOS.app.tar.gz")
        self.assertEqual(m["platforms"]["windows-x86_64"]["signature"], "win-sig")
        self.assertEqual(m["platforms"]["linux-x86_64"]["url"], URL + "/Password365-Linux.AppImage")

    def test_a_missing_platform_is_left_out(self):
        m = manifest("0.2.0", ["Password365-macOS.app.tar.gz"], SIGS.__getitem__, URL, now=NOW)
        self.assertEqual(sorted(m["platforms"]), ["darwin-aarch64", "darwin-x86_64"])

    def test_nothing_or_ambiguous_bundles_are_refused(self):
        with self.assertRaises(ValueError):
            manifest("0.2.0", ["readme.txt"], SIGS.__getitem__, URL, now=NOW)
        with self.assertRaises(ValueError):
            manifest("0.2.0", ["a-setup.exe", "b-setup.exe"], lambda _: "s", URL, now=NOW)


class Notes(unittest.TestCase):
    def test_the_versions_section_only(self):
        self.assertEqual(notes(CHANGELOG, "0.2.0"), "### Added\n- Skip a version.")
        self.assertEqual(notes(CHANGELOG, "0.1.0"), "- First release.")

    def test_an_unknown_version_has_no_notes(self):
        self.assertEqual(notes(CHANGELOG, "9.9.9"), "")


if __name__ == "__main__":
    unittest.main()
