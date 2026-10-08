#!/usr/bin/env python3
"""Build matched SQLite from its official amalgamation, then optionally Cargo."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent
GENERATED = ROOT / "generated"
URL = "https://sqlite.org/2026/sqlite-amalgamation-3530400.zip"
VERSION = "3.53.4"
ARCHIVE_SHA256 = "1e71ddf93849c6a6ecf58b827c0692073d2dd7ee40196158068f7b29f422e87d"
FLAGS = [
    "-O2", "-fPIC", "-pthread", "-DSQLITE_THREADSAFE=1", "-DSQLITE_DQS=0",
    "-DSQLITE_DEFAULT_WAL_SYNCHRONOUS=1", "-DSQLITE_LIKE_DOESNT_MATCH_BLOBS",
    "-DSQLITE_STRICT_SUBTYPE=1", "-DSQLITE_OMIT_DEPRECATED", "-DSQLITE_OMIT_SHARED_CACHE",
    "-DSQLITE_USE_URI=1", "-DSQLITE_ALLOW_URI_AUTHORITY", "-DSQLITE_DEFAULT_FOREIGN_KEYS=1",
    "-DSQLITE_TRUSTED_SCHEMA=0", "-DSQLITE_ENABLE_API_ARMOR", "-DSQLITE_ENABLE_ATOMIC_WRITE",
    "-DSQLITE_ENABLE_BATCH_ATOMIC_WRITE", "-DSQLITE_ENABLE_COLUMN_METADATA",
    "-DSQLITE_ENABLE_MATH_FUNCTIONS", "-DSQLITE_ENABLE_PREUPDATE_HOOK",
    "-DSQLITE_ENABLE_SETLK_TIMEOUT=2", "-DSQLITE_ENABLE_STAT4=1", "-DSQLITE_SOUNDEX",
    "-DSQLITE_OMIT_COMPLETE", "-DSQLITE_OMIT_GET_TABLE", "-DSQLITE_OMIT_DESERIALIZE",
    "-DSQLITE_OMIT_LOAD_EXTENSION", "-DSQLITE_MAX_MMAP_SIZE=1073741824",
    "-DSQLITE_EXPERIMENTAL_PRAGMA_20251114",
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo", action="store_true", help="also build the Rust benchmark release")
    arguments = parser.parse_args()
    GENERATED.mkdir(parents=True, exist_ok=True)
    archive = GENERATED / "sqlite-amalgamation-3530400.zip"
    if not archive.exists():
        temporary = archive.with_suffix(".download")
        with urllib.request.urlopen(URL, timeout=60) as response, temporary.open("wb") as output:
            while chunk := response.read(1 << 20):
                output.write(chunk)
        temporary.replace(archive)
    archive_sha = hashlib.sha256(archive.read_bytes()).hexdigest()
    if archive_sha != ARCHIVE_SHA256:
        raise RuntimeError(f"SQLite archive SHA256 mismatch: {archive_sha}")
    with zipfile.ZipFile(archive) as zipped:
        for member in zipped.infolist():
            destination = (GENERATED / member.filename).resolve()
            if not destination.is_relative_to(GENERATED.resolve()):
                raise RuntimeError("amalgamation archive member escapes generated directory")
        zipped.extractall(GENERATED)
    include = GENERATED / "sqlite-amalgamation-3530400"
    library = GENERATED / "lib"
    library.mkdir(exist_ok=True)
    compiler = shlex.split(os.environ.get("CC", "cc"))
    archiver = shlex.split(os.environ.get("AR", "ar"))
    subprocess.run(compiler + FLAGS + ["-c", str(include / "sqlite3.c"), "-o", str(library / "sqlite3.o")], check=True)
    subprocess.run(archiver + ["rcs", str(library / "libsqlite3.a"), str(library / "sqlite3.o")], check=True)
    metadata = {
        "version": VERSION, "download_url": URL, "archive_sha256": archive_sha,
        "sqlite3_c_sha256": hashlib.sha256((include / "sqlite3.c").read_bytes()).hexdigest(),
        "sqlite3_h_sha256": hashlib.sha256((include / "sqlite3.h").read_bytes()).hexdigest(),
        "compiler": subprocess.check_output(compiler + ["--version"], text=True).splitlines()[0],
        "flags": FLAGS,
        "intentional_differences": ["native THREADSAFE=1", "native Unix VFS", "native automatic initialization"],
    }
    (GENERATED / "native-build.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps(metadata, indent=2), flush=True)
    if arguments.cargo:
        environment = os.environ.copy()
        environment.update(SQLITE3_LIB_DIR=str(library), SQLITE3_INCLUDE_DIR=str(include), SQLITE3_STATIC="1")
        subprocess.run(["cargo", "build", "--release", "--locked"], cwd=ROOT / "rust", env=environment, check=True)


if __name__ == "__main__":
    main()
