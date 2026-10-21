#!/usr/bin/env python3
"""Stage reviewed package ZIPs with one combined download checksum manifest."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import unicodedata

# Reuse the transport packager's portable names and bounded plain-file hashing.
spec = importlib.util.spec_from_file_location("weftora_transport", Path(__file__).with_name("package-folder.py"))
transport = importlib.util.module_from_spec(spec)
spec.loader.exec_module(transport)
require = transport.require

MAX_PACKAGES = 16
MAX_ASSET_BYTES = 2 * 1024 * 1024 * 1024
MAX_TOTAL_BYTES = 4 * 1024 * 1024 * 1024
MAX_SUMS_BYTES = 4096


def signature(info):
    return info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns


def checksum_bytes(path):
    info = transport.plain_stat(path)
    require(stat.S_ISREG(info.st_mode) and info.st_size <= MAX_SUMS_BYTES, "invalid package SHA256SUMS size/type")
    with path.open("rb") as source:
        data = source.read(MAX_SUMS_BYTES + 1)
    require(len(data) <= MAX_SUMS_BYTES, "package SHA256SUMS grew beyond limit")
    return data


def package_record(folder):
    require(stat.S_ISDIR(transport.plain_stat(folder).st_mode), "package input must be a directory")
    folder = folder.resolve(strict=True)
    sums = checksum_bytes(folder / "SHA256SUMS")
    lines = sums.decode("utf-8").splitlines()
    match = re.fullmatch(r"([0-9a-f]{64})  (.+)", lines[0]) if len(lines) == 1 else None
    require(match is not None, "package SHA256SUMS must list exactly one ZIP")
    expected, name = match.groups()
    transport.safe_name(name)
    require("/" not in name and name.endswith(".zip"), "package checksum must name one ZIP basename")
    archives = [path.name for path in folder.iterdir() if path.name.lower().endswith(".zip")]
    require(archives == [name], "package directory must contain exactly the listed ZIP")
    source = folder / name
    info = transport.plain_stat(source)
    require(stat.S_ISREG(info.st_mode) and info.st_size <= MAX_ASSET_BYTES, "ZIP size/type exceeds staging limits")
    return {"source": source, "sums": sums, "signature": signature(info),
            "name": name, "size": info.st_size, "sha256": expected}


def verify_source(record):
    source = record["source"]
    require(signature(transport.plain_stat(source)) == record["signature"], "package ZIP changed during staging")
    require(checksum_bytes(source.parent / "SHA256SUMS") == record["sums"], "package checksums changed during staging")
    require(transport.file_hash(source, record["size"]) == record["sha256"], "package ZIP checksum mismatch")
    require(signature(transport.plain_stat(source)) == record["signature"], "package ZIP changed while hashing")


def stage(packages, output):
    require(0 < len(packages) <= MAX_PACKAGES, "stage between 1 and 16 package directories")
    output = output.absolute()
    require(not os.path.lexists(output), "release asset output must be fresh")
    output = output.resolve()
    records = [package_record(folder) for folder in packages]
    require(all(not output.is_relative_to(record["source"].parent) for record in records), "package/output overlap")
    aliases = [unicodedata.normalize("NFC", record["name"]).casefold() for record in records]
    require(len(set(aliases)) == len(aliases), "duplicate/aliased release asset names")
    require(sum(record["size"] for record in records) <= MAX_TOTAL_BYTES, "total release asset bytes exceed limit")
    records.sort(key=lambda record: record["name"])
    for record in records:
        verify_source(record)
    output.mkdir(parents=True, exist_ok=False)
    for record in records:
        written = 0
        with record["source"].open("rb") as source, (output / record["name"]).open("xb") as destination:
            for chunk in iter(lambda: source.read(transport.CHUNK), b""):
                written += len(chunk)
                require(written <= record["size"], "package ZIP grew during copy")
                destination.write(chunk)
        require(written == record["size"], "package ZIP shrank during copy")
    for record in records:
        verify_source(record)
        require(transport.file_hash(output / record["name"], record["size"]) == record["sha256"],
                "staged ZIP checksum mismatch")
    inventory = [{"name": record["name"], "size": record["size"], "sha256": record["sha256"],
                  "sourcePackage": record["source"].parent.name} for record in records]
    (output / "release-assets.json").write_bytes((json.dumps(inventory, indent=2) + "\n").encode("utf-8"))
    # Success manifest is written only after every source and staged ZIP verifies.
    (output / "SHA256SUMS").write_bytes("".join(f'{record["sha256"]}  {record["name"]}\n' for record in records).encode("utf-8"))
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, action="append", type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        print(stage(args.package, args.out))
    except (ValueError, OSError) as error:
        parser.exit(1, f"release asset staging refused: {error}\n")


if __name__ == "__main__":
    main()
