"""Read-only SHA-256, source extraction and independent recorded-WAL checks.

This checks consistency of the released bundle, not its external authenticity.
No build, network access, repair or manifest regeneration is performed.
"""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import struct

ROOT = Path(__file__).resolve().parent.parent
EXCLUDED = {".git", "target", "run-evidence", "__pycache__"}
MANIFEST = "SHA256SUMS"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def released_files():
    result = set()
    for directory, folders, files in os.walk(ROOT, followlinks=False):
        for name in list(folders):
            candidate = Path(directory) / name
            if name in EXCLUDED:
                folders.remove(name)
            else:
                require(not candidate.is_symlink(), "directory symlink refused")
        for name in files:
            candidate = Path(directory) / name
            relative = candidate.relative_to(ROOT).as_posix()
            if relative == MANIFEST:
                continue
            require(not candidate.is_symlink(), "file symlink refused")
            result.add(relative)
    return result


def check_manifest():
    entries = {}
    for line in (ROOT / MANIFEST).read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        require(match is not None, "invalid SHA256SUMS line")
        digest, name = match.groups()
        path = PurePosixPath(name)
        require(not path.is_absolute() and ".." not in path.parts and ":" not in name
                and "\\" not in name and path.as_posix() == name, "unsafe path")
        require(name not in entries and name != MANIFEST, "duplicate/self entry")
        entries[name] = digest
    require(set(entries) == released_files(), "manifest file-set mismatch")
    for name, digest in entries.items():
        require(sha((ROOT / name).read_bytes()) == digest, "SHA-256 mismatch: " + name)
    return len(entries)


def check_extraction():
    metadata = json.loads((ROOT / "provenance/source_gate.json").read_bytes())
    data = (ROOT / "src/source_gate.rs").read_bytes()
    require(sha(data) == metadata["output_sha256"], "source gate whole-file hash")
    lines = data.splitlines(keepends=True)
    for block in metadata["retained_source_blocks"]:
        start, end = block["output_line_range_inclusive"]
        selected = b"".join(lines[start - 1:end])
        require(len(selected) == block["selected_bytes"]
                and sha(selected) == block["selected_sha256"], "selected source block mismatch")
    return len(metadata["retained_source_blocks"])


def check_recorded_wal():
    # Independent Python decoding of the released Rust process evidence.
    directory = ROOT / "evidence/five-cycles"
    data = (directory / "five-cycle-final.wal").read_bytes()
    resource = b"poc010-test-vault"
    require(data[:8] == b"POC010\0\x01", "WAL magic")
    require(data[8:40].hex() == sha(b"ESSMAI_POC010_RESOURCE_V1" + resource), "WAL resource")
    head = hashlib.sha256(data[:40]).digest()
    at, records, receipts = 40, [], {}
    while at < len(data):
        require(len(data) - at >= 36, "torn frame header")
        length = struct.unpack_from("<I", data, at)[0]
        require(0 < length <= 256 * 1024 and at + 36 + length <= len(data), "frame bounds")
        stored, body = data[at + 4:at + 36], data[at + 36:at + 36 + length]
        computed = hashlib.sha256(b"ESSMAI_POC010_TX_V1" + head + struct.pack("<I", length) + body).digest()
        require(stored == computed, "record chain or receipt mismatch")
        record = json.loads(body)
        canonical = json.dumps(record, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
        require(body == canonical, "noncanonical recorded JSON")
        require(record["version"] == 1 and record["kind"] == "claim_effect_commit"
                and record["sequence"] == len(records) + 1, "record schema/sequence")
        request = record["request"]
        require(request["resource"].encode() == resource, "record resource")
        identity = b"ESSMAI_POC010_ACTION_V1"
        for field in ["principal", "operation", "resource", "scope", "context", "request_id"]:
            encoded = request[field].encode()
            identity += struct.pack("<Q", len(encoded)) + encoded
        action = sha(identity)
        require(action not in receipts, "duplicate physical action")
        receipts[action] = stored.hex()
        records.append(record)
        head, at = computed, at + 36 + length
    result = json.loads((directory / "five-cycle-results.json").read_bytes())
    require(len(records) == result["final_physical_effect_count"] == 3, "physical count")
    require([c["observed_effect_count"] for c in result["cycles"]] == [1, 1, 2, 3, 3], "cycle counts")
    for cycle in result["cycles"]:
        outcomes = cycle.get("worker_results", [cycle.get("result")])
        for outcome in outcomes:
            require(receipts[outcome["action_sha256"]] == outcome["receipt_sha256"], "reported receipt mismatch")
    return len(records)


if __name__ == "__main__":
    try:
        count = check_manifest()
        blocks = check_extraction()
        records = check_recorded_wal()
    except (ValueError, OSError, KeyError, TypeError) as error:
        raise SystemExit("FAIL: " + str(error))
    print(f"PASS: {count} artifact hashes; {blocks} exact source blocks; independent WAL replay: {records} records; five cycle counts 1,1,2,3,3")
