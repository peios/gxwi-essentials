"""Check produced package boundaries, source completeness and shared versions."""

import json
import tomllib
import unittest
from pathlib import Path
from typing import Any, ClassVar

APPLICATIONS = {
    "gxwi-disk-manager",
    "gxwi-event-viewer",
    "gxwi-feature-manager",
    "gxwi-file-dialog",
    "gxwi-file-explorer",
    "gxwi-my-settings",
    "gxwi-network-manager",
    "gxwi-package-manager",
    "gxwi-principals-manager",
    "gxwi-registry-editor",
    "gxwi-sd-editor",
    "gxwi-security-policy",
    "gxwi-services-manager",
    "gxwi-system-settings",
    "gxwi-task-manager",
    "gxwi-terminal",
    "gxwi-text-editor",
    "gxwi-upgrade-peios",
}


class FamilyTests(unittest.TestCase):
    """Inspect canonical manifests and provenance from the completed package store."""

    packages: ClassVar[dict[str, Any]]
    manifests: ClassVar[dict[str, Any]]
    index: ClassVar[dict[str, Any]]

    @classmethod
    def setUpClass(cls) -> None:
        root = Path("packages.store")
        cls.index = json.loads((root / "index.json").read_text())
        cls.packages = {p["name"].removeprefix("dev.peios."): p for p in cls.index["packages"]}
        cls.manifests = {
            name: tomllib.loads((root / p["manifest"]).read_text())["files"]
            for name, p in cls.packages.items()
        }

    def test_complete_unsigned_family(self) -> None:
        expected = {
            name + suffix for name in APPLICATIONS for suffix in ("", "-debuginfo", "-debugsource")
        } | {"gxwi-essentials-source"}
        self.assertEqual(set(self.packages), expected)
        self.assertEqual(self.index["package_count"], 55)
        self.assertEqual(len({p["version"] for p in self.packages.values()}), 1)
        self.assertFalse(self.index["release_ready"])
        self.assertEqual(self.index["trust"], "development")
        for name, info in self.packages.items():
            self.assertFalse(info["release_ready"], name)
            # Debug sources include target-generated ABI bindings and match the binary architecture.
            arch = "noarch" if name == "gxwi-essentials-source" else "x86_64"
            self.assertEqual(info["arch"], arch, name)
            if "build" in info:
                self.assertFalse(info["build"]["signed"])
                self.assertIn(
                    info["build"]["userspace_abi"], {"peios-native", "debian-development"}
                )

    def test_each_application_has_its_own_runtime(self) -> None:
        for name, files in self.manifests.items():
            if name == "gxwi-essentials-source":
                continue
            executables = {p for p, entry in files.items() if entry.get("exec")}
            self.assertEqual(executables, {"usr/bin/" + name} if name in APPLICATIONS else set())
            self.assertFalse(any(p.startswith("var/") for p in files))
            self.assertFalse(any("symlink" in entry for entry in files.values()))
        for name in APPLICATIONS:
            files = self.manifests[name]
            required = {
                "usr/bin/" + name,
                "usr/share/apps/dev.peios." + name + ".toml",
                "usr/share/icons/base/dev.peios." + name + ".svg",
                "usr/share/man/man1/" + name + ".1.gz",
                "usr/share/licenses/dev.peios." + name + "/LICENSE",
                "usr/share/licenses/dev.peios." + name + "/third-party/crate-licenses.tsv",
            }
            self.assertTrue(required <= files.keys(), (name, required - files.keys()))

    def test_debug_pairs_and_suite_dependencies_are_exact(self) -> None:
        for name in APPLICATIONS:
            runtime = self.packages[name]
            for suffix in ("-debuginfo", "-debugsource"):
                info = self.packages[name + suffix]
                self.assertIn(
                    "usr/share/licenses/dev.peios."
                    + name
                    + suffix
                    + "/third-party/crate-licenses.tsv",
                    self.manifests[name + suffix],
                )
                self.assertEqual(
                    info["package_metadata"]["dependencies"],
                    {
                        "dev.peios." + name: "= " + runtime["version"],
                    },
                )
                self.assertEqual(
                    info["package_metadata"]["package"]["license"],
                    runtime["package_metadata"]["package"]["license"],
                )
            symbols = [p for p in self.manifests[name + "-debuginfo"] if p.endswith(".debug")]
            self.assertEqual(len(symbols), 1, name)
            self.assertRegex(symbols[0], r"^usr/lib/debug/\.build-id/[0-9a-f]{2}/[0-9a-f]+\.debug$")
            source = self.manifests[name + "-debugsource"]
            self.assertTrue(any(p.endswith("/apps/" + name + "/src/main.rs") for p in source))
            for dependency, constraint in (
                runtime["package_metadata"].get("dependencies", {}).items()
            ):
                if dependency.removeprefix("dev.peios.") in APPLICATIONS:
                    self.assertEqual(constraint, "= " + runtime["version"])
            self.assertNotIn("sd_overrides", runtime["package_metadata"])

    def test_source_package_is_rebuildable_and_clean(self) -> None:
        files = self.manifests["gxwi-essentials-source"]
        required = {
            "Cargo.lock",
            "Cargo.toml",
            "MODULE.bazel",
            "MODULE.bazel.lock",
            "BUILD.bazel",
            "bazel",
            "packages/stage.py",
            "packages/validate.py",
            "vendor/ppk/src/lib.rs",
            "vendor/kernel/source-sha256.json",
            "third-party/source-notices.json",
            "crates/gxwi-fs/src/lib.rs",
            "crates/gxwi-process/src/lib.rs",
        }
        required.update("apps/" + name + "/src/main.rs" for name in APPLICATIONS)
        self.assertTrue({"usr/src/gxwi-essentials/" + p for p in required} <= files.keys())
        forbidden = {
            ".git",
            ".trail-cache",
            ".ruff_cache",
            "__pycache__",
            "target",
            "bazel-out",
            "packages.pekit",
        }
        for path in files:
            self.assertFalse(forbidden & set(Path(path).parts), path)
            self.assertNotEqual(Path(path).name, "pekit.toml")


if __name__ == "__main__":
    unittest.main()
