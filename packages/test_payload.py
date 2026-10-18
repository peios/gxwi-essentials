"""Exercise payload gates against the declared application resources."""

import tempfile
import unittest
from pathlib import Path

from stage import applications, stage_data
from validate import validate_data


class PayloadTests(unittest.TestCase):
    """Reject broken registrations, missing resources and mutable installed state."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        sources = list(Path("docs/man").glob("*.1")) + list(Path("docs").glob("*.regman"))
        for name in applications():
            app = Path("apps") / name
            sources.extend(path for path in app.rglob("*") if path.is_file())
        stage_data(sources, self.root)

    def test_all_application_resources_pass(self) -> None:
        for name in applications():
            with self.subTest(name=name):
                validate_data(self.root, name)

    def test_wrong_launcher_target_is_rejected(self) -> None:
        name = "gxwi-terminal"
        app = self.root / "runtime" / name / "usr/share/apps" / ("dev.peios." + name + ".toml")
        app.write_text(app.read_text().replace("/usr/bin/" + name, "/usr/bin/another-app"))
        with self.assertRaisesRegex(ValueError, "wrong executable"):
            validate_data(self.root, name)

    def test_missing_artwork_is_rejected(self) -> None:
        name = "gxwi-text-editor"
        icon = (
            self.root / "runtime" / name / "usr/share/icons/base" / ("dev.peios." + name + ".svg")
        )
        icon.unlink()
        with self.assertRaisesRegex(ValueError, "missing icon"):
            validate_data(self.root, name)

    def test_install_does_not_include_user_state(self) -> None:
        name = "gxwi-file-explorer"
        (self.root / "runtime" / name / "var").mkdir()
        with self.assertRaisesRegex(ValueError, "mutable state"):
            validate_data(self.root, name)

    def test_registry_seed_changes_are_rejected(self) -> None:
        for seed in Path("apps").glob("*/registry.d/*"):
            name = seed.parts[1]
            installed = self.root / "runtime" / name / "usr/share/regim" / seed.name
            installed.write_bytes(b"changed registry authority")
            with self.assertRaisesRegex(ValueError, "seed policy changed"):
                validate_data(self.root, name)
            installed.write_bytes(seed.read_bytes())


if __name__ == "__main__":
    unittest.main()
