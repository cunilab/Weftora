#!/usr/bin/env python3
"""Package a reviewed, quiescent folder and verify its ZIP transport round trip."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import unicodedata
import zipfile

MAX_ENTRIES = 10_000  # Includes directories and the package root.
MAX_BYTES = 1024 * 1024 * 1024
CHUNK = 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def safe_name(name):
    require(name and len(name.encode("utf-8")) <= 1024, f"invalid member name: {name!r}")
    for part in name.split("/"):
        require(part not in ("", ".", "..") and not part.endswith((".", " "))
                and not re.search(r'[<>:"\\|?*\x00-\x1f]', part), f"unsafe member name: {name!r}")
        require(not re.fullmatch(r"CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|COM[1-9¹²³]|LPT[1-9¹²³]", part.partition(".")[0].rstrip(" "), re.IGNORECASE),
                f"reserved Windows name: {name!r}")
    return name


def plain_stat(path):
    result = path.lstat()
    require(not stat.S_ISLNK(result.st_mode) and not getattr(result, "st_file_attributes", 0) & 0x400,
            f"links/reparse points are not supported: {path}")
    require(stat.S_ISREG(result.st_mode) or stat.S_ISDIR(result.st_mode), f"not a regular file/directory: {path}")
    return result


def file_hash(path, expected_size=None):
    result = hashlib.sha256()
    count = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(CHUNK), b""):
            count += len(chunk)
            require(expected_size is None or count <= expected_size, f"file grew while hashing: {path}")
            result.update(chunk)
    require(expected_size is None or count == expected_size, f"file shrank while hashing: {path}")
    return result.hexdigest()


def walk_error(error):
    raise error


def inventory(folder):
    require(stat.S_ISDIR(plain_stat(folder).st_mode), "package source must be a directory")
    records, aliases, total = {}, set(), 0
    for directory, directories, files in os.walk(folder, followlinks=False, onerror=walk_error):
        for path in [Path(directory)] + [Path(directory) / name for name in sorted(files)]:
            info = plain_stat(path)
            name = safe_name(path.relative_to(folder.parent).as_posix())
            alias = unicodedata.normalize("NFC", name).casefold()
            require(alias not in aliases, f"case-colliding member: {name}")
            aliases.add(alias)
            require(len(records) < MAX_ENTRIES, "package entry count exceeds limit")
            if stat.S_ISDIR(info.st_mode):
                record = {"directory": True}
            else:
                total += info.st_size
                require(total <= MAX_BYTES, "package uncompressed bytes exceed limit")
                record = {"size": info.st_size, "sha256": file_hash(path, info.st_size), "executable": bool(info.st_mode & 0o111)}
            records[name] = record
        # os.walk does not descend into symlink directories; refuse them explicitly.
        for name in directories:
            plain_stat(Path(directory) / name)
    require(any(not record.get("directory") for record in records.values()), "package contains no files")
    return dict(sorted(records.items()))


def write_zip(folder, archive, approved):
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED, allowZip64=False) as bundle:
        for name, record in approved.items():
            path = folder.parent / name
            info = plain_stat(path)
            directory = record.get("directory", False)
            require(stat.S_ISDIR(info.st_mode) == directory, f"source type changed: {name}")
            entry = zipfile.ZipInfo(name + ("/" if directory else ""))
            entry.create_system = 3
            mode = (stat.S_IFDIR | 0o755) if directory else (stat.S_IFREG | (0o755 if record["executable"] else 0o644))
            entry.external_attr = (mode << 16) | (0x10 if directory else 0)
            entry.compress_type = zipfile.ZIP_DEFLATED
            with bundle.open(entry, "w") as destination:
                if not directory:
                    with path.open("rb") as source:
                        written = 0
                        for chunk in iter(lambda: source.read(CHUNK), b""):
                            written += len(chunk)
                            require(written <= record["size"], f"source grew while packaging: {name}")
                            destination.write(chunk)


def verify_zip(archive, destination, approved):
    """Validate our generated transport against its frozen inventory before writing."""
    require(not os.path.lexists(destination), "verification destination must be fresh")
    require(len(approved) <= MAX_ENTRIES, "approved entry count exceeds limit")
    require(sum(record.get("size", 0) for record in approved.values()) <= MAX_BYTES, "approved bytes exceed limit")
    with zipfile.ZipFile(archive) as bundle:
        entries = bundle.infolist()
        require(len(entries) <= MAX_ENTRIES, "ZIP entry count exceeds limit")
        seen, total = set(), 0
        for entry in entries:
            # orig_filename retains NULs that ZipInfo otherwise silently truncates.
            raw = entry.orig_filename
            name = safe_name(raw[:-1] if raw.endswith("/") else raw)
            require(name not in seen, f"duplicate ZIP member: {name}")
            seen.add(name)
            require(name in approved, f"unexpected ZIP member: {name}")
            record = approved[name]
            directory = record.get("directory", False)
            expected_mode = (stat.S_IFDIR | 0o755) if directory else (stat.S_IFREG | (0o755 if record["executable"] else 0o644))
            require(entry.create_system == 3
                    and entry.external_attr == ((expected_mode << 16) | (0x10 if directory else 0))
                    and not entry.extra and entry.is_dir() == directory,
                    f"ZIP member type/mode mismatch: {name}")
            require(not entry.flag_bits & 1 and entry.compress_type == zipfile.ZIP_DEFLATED,
                    f"unsupported ZIP encoding: {name}")
            require(entry.file_size == record.get("size", 0), f"ZIP member size mismatch: {name}")
            total += entry.file_size
            require(total <= MAX_BYTES, "ZIP uncompressed bytes exceed limit")
        require(seen == set(approved), "ZIP inventory does not match approved folder")
        destination.mkdir(parents=True, exist_ok=False)
        for entry in entries:
            name = entry.filename.rstrip("/")
            path, record = destination / name, approved[name]
            if record.get("directory"):
                path.mkdir(parents=True, exist_ok=True)
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            digest, written = hashlib.sha256(), 0
            with bundle.open(entry) as source, path.open("xb") as output:
                for chunk in iter(lambda: source.read(CHUNK), b""):
                    written += len(chunk)
                    require(written <= record["size"], f"ZIP member exceeds approved size: {name}")
                    digest.update(chunk)
                    output.write(chunk)
            require(written == record["size"] and digest.hexdigest() == record["sha256"], f"ZIP content mismatch: {name}")
            path.chmod(0o755 if record["executable"] else 0o644)
    root = next(iter(approved)).split("/")[0]
    require(inventory(destination / root) == approved, "extracted inventory differs from approved folder")


def package(folder, output):
    plain_stat(folder)
    folder, output = folder.resolve(strict=True), output.absolute()
    require(not os.path.lexists(output), "package output must be fresh")
    output = output.resolve()
    require(not output.is_relative_to(folder) and not folder.is_relative_to(output), "source/output overlap")
    approved = inventory(folder)
    output.mkdir(parents=True, exist_ok=False)
    archive = output / (folder.name + ".zip")
    write_zip(folder, archive, approved)
    verify_zip(archive, output / "verified", approved)
    require(inventory(folder) == approved, "source changed during packaging")
    (output / "folder-inventory.json").write_text(json.dumps(approved, indent=2) + "\n", encoding="utf-8")
    checksum = file_hash(archive)
    report = {"zip": archive.name, "sha256": checksum, "entries": len(approved),
              "uncompressedBytes": sum(record.get("size", 0) for record in approved.values()),
              "maxEntries": MAX_ENTRIES, "maxUncompressedBytes": MAX_BYTES,
              "inventorySha256": file_hash(output / "folder-inventory.json"), "roundTripVerified": True,
              "scope": "Transport of a reviewed, quiescent folder; not Engine archive support, licence approval or native/clean-machine acceptance."}
    (output / "transport-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "SHA256SUMS").write_text(f"{checksum}  {archive.name}\n", encoding="utf-8")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        print(package(args.source, args.out))
    except (ValueError, OSError, zipfile.BadZipFile, zipfile.LargeZipFile) as error:
        parser.exit(1, f"packaging refused: {error}\n")


if __name__ == "__main__":
    main()
