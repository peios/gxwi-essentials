#!/usr/bin/python3
"""Stage and validate declared GXWI application artifacts before writing any PPK commit."""

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import TypedDict, cast

sys.dont_write_bytecode = True
from licences import collect  # noqa: E402
from validate import validate  # noqa: E402


class ComponentInput(TypedDict):
    """Declared binary and transitive source inputs from the staging rule."""

    binary: str
    inputs: list[str]


class StageSpec(TypedDict):
    """Internal staging manifest emitted by packages/build.bzl."""

    components: dict[str, ComponentInput]
    sources: list[str]
    version: str
    sdk: str
    rustc: str
    native: bool


def applications() -> list[str]:
    """Read the shared workspace's explicit application membership."""
    workspace = tomllib.loads(Path("Cargo.toml").read_text())["workspace"]
    return sorted(
        Path(member).name for member in workspace["members"] if member.startswith("apps/")
    )


def copy(source: Path, target: Path) -> None:
    """Copy a file while normalizing its mode for an installed payload."""
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    target.chmod(0o755 if source.stat().st_mode & 0o111 else 0o644)


def split_debug(component: str, elf: Path, output: Path) -> None:
    """Split ELF debug data and record the GNU build ID and debug link."""
    notes = subprocess.check_output(["readelf", "-n", str(elf)], text=True)
    match = re.search(r"Build ID: ([0-9a-f]+)", notes)
    if not match:
        raise ValueError(f"missing build ID: {elf}")
    build_id = match.group(1)
    debug = (
        output
        / f"debuginfo/{component}/usr/lib/debug/.build-id/{build_id[:2]}/{build_id[2:]}.debug"
    )
    debug.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["objcopy", "--only-keep-debug", elf, debug], check=True)
    subprocess.run(["strip", "--strip-unneeded", elf], check=True)
    subprocess.run(["objcopy", f"--add-gnu-debuglink={debug}", elf], check=True)


def source_files(inputs: list[Path]) -> list[Path]:
    """Resolve declared source files and generated bindings, rejecting missing inputs."""
    files = set()
    for path in inputs:
        if path.is_dir():
            # Generated bindings and build-script source products, not compiled archives.
            files.update(
                p for p in path.rglob("*") if p.is_file() and p.suffix in {".rs", ".h", ".c"}
            )
        elif path.is_file():
            files.add(path)
        else:
            raise ValueError(f"missing declared input: {path}")
    return sorted(files)


def stage_data(sources: list[Path], output: Path) -> None:
    """Install each application's own registration, artwork, manual and inert seeds."""
    for name in applications():
        payload = output / "runtime" / name
        app = Path("apps") / name
        for source in sources:
            if source == Path("docs/man") / (name + ".1"):
                target = payload / "usr/share/man/man1" / (source.name + ".gz")
                target.parent.mkdir(parents=True, exist_ok=True)
                with target.open("wb") as stream:
                    subprocess.run(
                        ["gzip", "-n", "-9", "-c", str(source)], stdout=stream, check=True
                    )
            elif source == Path("docs") / (name + ".regman"):
                copy(source, payload / "usr/share/regman" / source.name)
            elif source == app / ("dev.peios." + name + ".toml"):
                copy(source, payload / "usr/share/apps" / source.name)
            elif source == app / (name + ".svg"):
                copy(source, payload / "usr/share/icons/base" / ("dev.peios." + source.name))
            elif source.parent == app / "registry.d":
                copy(source, payload / "usr/share/regim" / source.name)
            elif source.parent == app / "third-party":
                copy(
                    source,
                    payload
                    / "usr/share/licenses"
                    / ("dev.peios." + name)
                    / "bundled"
                    / source.name,
                )


def check_versions(sources: list[Path], version: str) -> None:
    """Require every member to inherit the one declared suite version."""
    workspace = tomllib.loads(Path("Cargo.toml").read_text())["workspace"]
    if workspace["package"]["version"] != version:
        raise ValueError("workspace version differs from Bazel")
    for member in workspace["members"]:
        if member.startswith("vendor/"):
            continue
        manifest = Path(member) / "Cargo.toml"
        if manifest not in sources:
            raise ValueError(f"missing workspace manifest: {manifest}")
        if tomllib.loads(manifest.read_text())["package"]["version"] != {"workspace": True}:
            raise ValueError(f"member does not inherit suite version: {manifest}")


def _stage_components(
    components: dict[str, ComponentInput], output: Path, sources: list[Path], sdk: Path
) -> tuple[dict[str, str], set[Path]]:
    if set(components) != set(applications()):
        raise ValueError("incomplete or unexpected suite binary set")
    expressions = {}
    all_inputs = set(sources)
    declared = set(sources)
    for name, component in sorted(components.items()):
        payload = output / "runtime" / name
        elf = payload / "usr/bin" / name
        copy(Path(component["binary"]), elf)
        split_debug(name, elf, output)
        inputs = set(source_files([Path(path) for path in component["inputs"]]))
        # Workspace metadata and exact-source notices are explicit inputs.
        for source in list(inputs):
            for parent in source.parents:
                manifest = parent / "Cargo.toml"
                if manifest in declared:
                    inputs.add(manifest)
        inputs.add(Path("Cargo.toml"))
        inputs.update(p for p in sources if p.parts[0] == "third-party")
        inputs.update(
            p for p in sources if p.parent == Path("vendor/kernel") and p.name.startswith("LICENSE")
        )
        expressions[name] = collect(
            sorted(inputs), payload / "usr/share/licenses" / ("dev.peios." + name) / "third-party"
        )
        _stage_debug_sources(inputs, output / "debugsource" / name, sdk)
        all_inputs.update(inputs)
    return expressions, all_inputs


def _stage_debug_sources(debug_inputs: set[Path], output: Path, sdk: Path) -> None:
    # Only source inputs declared by Bazel enter debugsource; never walk a checkout or cache.
    debug_root = output / "usr/src/debug/dev.peios.gxwi-essentials"
    for source in sorted(debug_inputs):
        if source.is_relative_to(sdk) or source.suffix in {".so", ".a", ".rlib", ".o"}:
            continue
        if source.is_absolute() or ".." in source.parts:
            raise ValueError(f"non-relative declared source: {source}")
        copy(source, debug_root / source)


def _check_licences(expressions: dict[str, str]) -> None:
    for path in sorted(Path("packages/definitions").glob("*.json")):
        definition = json.loads(path.read_text())
        component = definition["package"]["name"].removeprefix("dev.peios.")
        component = component.removesuffix("-debuginfo").removesuffix("-debugsource")
        if definition["package"]["license"] != expressions[component]:
            raise ValueError(f"{path}: licence expression differs: {expressions[component]}")


def _write_build_record(
    spec: StageSpec, output: Path, debug_inputs: set[Path], expressions: dict[str, str]
) -> None:
    fingerprints = {
        str(path): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(debug_inputs)
        if path.is_file()
    }
    record = {
        "version": spec["version"],
        "userspace_abi": "peios-native" if spec["native"] else "debian-development",
        "release_ready": False,
        "signed": False,
        "checks": "native-hardening-and-payload" if spec["native"] else "development-payload",
        "input_digest": hashlib.sha256(
            json.dumps(fingerprints, sort_keys=True).encode("utf-8")
        ).hexdigest(),
        "licences": expressions,
    }
    (output / "build.json").write_text(
        json.dumps(record, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )


def main() -> None:
    """Stage the declared build inputs and write provenance after validation succeeds."""
    spec = cast(StageSpec, json.loads(Path(sys.argv[1]).read_text(encoding="utf-8")))
    output = Path(sys.argv[2])
    output.mkdir(parents=True, exist_ok=True)
    sources = [Path(path) for path in spec["sources"]]
    check_versions(sources, spec["version"])
    expressions, debug_inputs = _stage_components(
        spec["components"], output, sources, Path(spec["sdk"])
    )
    _check_licences(expressions)
    stage_data(sources, output)
    os.environ["RUSTC"] = str(Path(spec["rustc"]).absolute())
    validate(output, native=spec["native"], sdk=Path(spec["sdk"]))
    _write_build_record(spec, output, debug_inputs, expressions)


if __name__ == "__main__":
    main()
