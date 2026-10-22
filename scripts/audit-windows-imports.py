#!/usr/bin/env python3
"""Record ordinary DLL imports from frozen Windows package folders without running them."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import stat
import struct
import subprocess

spec = importlib.util.spec_from_file_location("transport", Path(__file__).with_name("package-folder.py"))
transport = importlib.util.module_from_spec(spec)
spec.loader.exec_module(transport)
require = transport.require


def imports(text):
    require("file format coff-x86-64" in text and re.search(r"(?m)^Magic\s+020b\b", text),
            "expected LLVM AMD64 PE32+ private headers")
    directories = {}
    for index, address, size in re.findall(r"(?m)^Entry ([0-9a-f]+) ([0-9a-f]+) ([0-9a-f]+) ", text):
        require(index not in directories, "duplicate PE data directory")
        directories[index] = (int(address, 16), int(size, 16))
    require("1" in directories and "d" in directories, "missing import/delay directory evidence")
    require(directories["d"] == (0, 0), "delay imports are not covered by this LLVM inspection")
    names = re.findall(r"(?m)^\s+DLL Name: ([A-Za-z0-9_.-]+)\s*$", text)
    require(len(names) == text.count("DLL Name:"), "unsupported import name/output")
    require(all("/" not in transport.safe_name(name) for name in names), "import name must be a basename")
    require(bool(names) == (directories["1"] != (0, 0)), "import directory and DLL evidence disagree")
    return sorted({name.lower() for name in names})


def is_pe(path):
    with path.open("rb") as source:
        header = source.read(64)
        if header[:2] != b"MZ":
            require(path.suffix.lower() not in (".exe", ".dll"), "native filename lacks a DOS/PE header")
            return False
        require(len(header) == 64, "truncated DOS header")
        source.seek(struct.unpack_from("<I", header, 60)[0])
        require(source.read(6) == b"PE\0\0\x64\x86", "expected AMD64 PE signature")
    return True


def audit(packages, objdump, output):
    require(0 < len(packages) <= 16, "audit between 1 and 16 package folders")
    for package in packages:
        require(stat.S_ISDIR(transport.plain_stat(package).st_mode), "package must be a plain directory")
    packages = [package.resolve(strict=True) for package in packages]
    require(len({p.name.casefold() for p in packages}) == len(packages), "duplicate package labels")
    require(all(not a.is_relative_to(b) for a in packages for b in packages if a != b), "package folders overlap")
    require(not os.path.lexists(output), "audit output must be fresh")
    output = output.resolve()
    require(all(not output.is_relative_to(p) and not p.is_relative_to(output) for p in packages), "audit output/package overlap")
    require(all(not objdump.resolve(strict=True).is_relative_to(p) for p in packages), "inspector must be outside candidate packages")
    snapshots = [transport.inventory(package) for package in packages]
    version = subprocess.run([str(objdump), "--version"], check=True, capture_output=True, text=True, timeout=15).stdout
    require(len(version) <= 65536, "LLVM version output exceeds limit")
    version_lines = [line.strip() for line in version.splitlines() if re.search(r"\bLLVM version [0-9]", line)]
    require(len(version_lines) == 1, "LLVM version evidence is missing/ambiguous")
    output.mkdir(parents=True, exist_ok=False)
    analyses, records = {}, []
    for package, snapshot in zip(packages, snapshots):
        native = []
        for name, row in snapshot.items():
            if row.get("directory"):
                continue
            path = package.parent / name
            if not is_pe(path):
                continue
            checksum = row["sha256"]
            if checksum not in analyses:
                log = output / (checksum + ".imports.txt")
                with log.open("xb") as destination:
                    subprocess.run([str(objdump), "--private-headers", str(path)], check=True,
                                   stdout=destination, stderr=subprocess.STDOUT, timeout=30)
                require(log.stat().st_size <= 8 * 1024 * 1024, "LLVM output exceeds 8 MiB")
                analyses[checksum] = {"sha256": checksum, "ordinaryImports": imports(log.read_text(encoding="utf-8")),
                                      "rawOutput": log.name, "delayImportDirectoryAbsent": True}
            native.append({"path": name, "sha256": checksum, "bytes": row["size"]})
        require(native, "package contains no inspectable AMD64 PE files")
        inventory_hash = hashlib.sha256((json.dumps(snapshot, indent=2) + "\n").encode()).hexdigest()
        records.append({"package": package.name, "fileCount": sum(not r.get("directory") for r in snapshot.values()),
                        "folderInventorySha256": inventory_hash, "peFiles": native})
    require(all(transport.inventory(package) == snapshot for package, snapshot in zip(packages, snapshots)),
            "package changed during import inspection")
    report = {"inspectionHost": platform.system(), "llvmVersion": version_lines[0],
              "packages": records, "binaries": list(analyses.values()), "auditApproved": False,
              "scope": "Static ordinary-import evidence for MZ/AMD64 PE files in these frozen folders. No candidate executable is launched. Nonzero delay-import directories refuse. Does not resolve transitive imports, LoadLibrary/runtime-loaded modules, host availability, redistribution rights or native/clean-machine acceptance."}
    (output / "windows-imports.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, action="append", type=Path)
    parser.add_argument("--objdump", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        audit(args.package, args.objdump.resolve(strict=True), args.out)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"import audit refused: {error}\n")


if __name__ == "__main__":
    main()
