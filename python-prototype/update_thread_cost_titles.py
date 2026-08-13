#!/usr/bin/env python3
"""Append estimated rollout cost to selected persisted Codex thread titles.

Usage:
  update_thread_cost_titles.py --idle-minutes 15 --limit 500 --apply
  update_thread_cost_titles.py --idle-minutes 15 --reprice-before 2026-08-06T20:00:00Z --limit 500 --apply
  update_thread_cost_titles.py --thread-id THREAD_ID --apply
  update_thread_cost_titles.py --match-title "unique title text" --apply

The idle form updates eligible root sessions; --limit bounds a run and
--max-runtime accepts seconds or whole minutes with an m suffix (for example,
90 or 5m). --reprice-before reprocesses cost titles last written before one
fixed ISO date or timestamp, allowing repeated batches to resume from the
session index. Without --apply, the script only prints proposed changes.
Persisted cost titles are capped at 65 total characters; an older over-limit
cost title remains eligible for repair even when its high-water mark is current.

To schedule the idle form every five minutes, run:
  update_thread_cost_titles.py --install-launch-agent

That writes and loads ~/Library/LaunchAgents/com.openai.codex.thread-cost-titles.plist
for this script, current Python interpreter, and selected --codex-home. Its
runs use --idle-minutes 15, --limit 500, --max-runtime 4m, and --apply.
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import re
import sqlite3
import subprocess
import sys
import tempfile
import time
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from rollout_stats import build_index, rollout_type, tree_stats


COST_SUFFIX = re.compile(r"\s+·\s+\$[\d,]+(?:\.\d{2})?\+?$")
# Bound the complete persisted title, not just synthetic prompt text, so the
# variable-width cost suffix never pushes sidebar rows back past the limit.
TITLE_LIMIT = 65
LAUNCH_AGENT_LABEL = "com.openai.codex.thread-cost-titles"


def state_db_path(codex_home: Path) -> Path:
    for path in (codex_home / "state_5.sqlite", codex_home / "sqlite" / "state_5.sqlite"):
        if path.is_file():
            return path
    raise SystemExit(f"state database not found under {codex_home}")


def install_launch_agent(codex_home: Path) -> Path:
    import plistlib

    destination = Path.home() / "Library" / "LaunchAgents" / f"{LAUNCH_AGENT_LABEL}.plist"
    log_path = codex_home / "logs" / "thread-cost-titles.log"
    destination.parent.mkdir(parents=True, exist_ok=True)
    log_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "Label": LAUNCH_AGENT_LABEL,
        "ProgramArguments": [
            sys.executable,
            str(Path(__file__).resolve()),
            "--codex-home", str(codex_home),
            "--idle-minutes", "15",
            "--limit", "500",
            "--max-runtime", "4m",
            "--apply",
        ],
        "RunAtLoad": True,
        "StartInterval": 300,
        "ProcessType": "Background",
        "StandardOutPath": str(log_path),
        "StandardErrorPath": str(log_path),
    }
    with destination.open("wb") as plist:
        plistlib.dump(payload, plist)
    domain = f"gui/{os.getuid()}"
    subprocess.run(["launchctl", "bootout", f"{domain}/{LAUNCH_AGENT_LABEL}"], capture_output=True)
    subprocess.run(["launchctl", "bootstrap", domain, str(destination)], check=True)
    return destination


def latest_index_entries(codex_home: Path) -> Dict[str, Tuple[str, Optional[datetime]]]:
    entries: Dict[str, Tuple[str, Optional[datetime]]] = {}
    try:
        with (codex_home / "session_index.jsonl").open() as lines:
            for line in lines:
                try:
                    item = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if item.get("id") and item.get("thread_name"):
                    updated_at = item.get("updated_at")
                    try:
                        parsed_updated_at = datetime.fromisoformat(updated_at.replace("Z", "+00:00"))
                    except (AttributeError, ValueError):
                        parsed_updated_at = None
                    entries[item["id"]] = (item["thread_name"], parsed_updated_at)
    except FileNotFoundError:
        pass
    return entries


def latest_index_names(codex_home: Path) -> Dict[str, str]:
    return {thread_id: name for thread_id, (name, _) in latest_index_entries(codex_home).items()}


def selected_threads(
    connection: sqlite3.Connection,
    index_names: Dict[str, str],
    root_ids: set[str],
    thread_ids: List[str],
    title_matches: List[str],
) -> List[sqlite3.Row]:
    rows = connection.execute(
        "SELECT id, title, name, history_mode, first_user_message FROM threads"
    ).fetchall()
    selected: list[sqlite3.Row] = []
    rows = [row for row in rows if row["id"] in root_ids]
    by_id = {row["id"]: row for row in rows}
    for thread_id in thread_ids:
        if thread_id not in by_id:
            raise SystemExit(f"root thread id not found: {thread_id}")
        selected.append(by_id[thread_id])
    for match in title_matches:
        candidates = [
            row
            for row in rows
            if any(match.lower() in value.lower() for value in (row["title"], row["name"], index_names.get(row["id"])) if value)
        ]
        if len(candidates) != 1:
            detail = "none" if not candidates else ", ".join(row["id"] for row in candidates)
            raise SystemExit(f"title substring {match!r} resolved to {detail}")
        selected.append(candidates[0])
    return selected


def idle_root_threads(
    connection: sqlite3.Connection,
    index_entries: Dict[str, Tuple[str, Optional[datetime]]],
    root_ids: set[str],
    idle_minutes: int,
    limit: int,
    reprice_before: Optional[datetime] = None,
) -> List[sqlite3.Row]:
    now = datetime.now(timezone.utc)
    newest = int(now.timestamp() - idle_minutes * 60)
    rows = connection.execute(
        """SELECT id, title, name, history_mode, updated_at, first_user_message
           FROM threads
           WHERE updated_at <= ?
           ORDER BY updated_at DESC""",
        (newest,),
    ).fetchall()
    selected = []
    for row in rows:
        if row["id"] not in root_ids:
            continue
        indexed = index_entries.get(row["id"])
        session_updated_at = datetime.fromtimestamp(row["updated_at"], timezone.utc)
        # The latest index timestamp is both the normal session high-water mark
        # and the checkpoint for a fixed --reprice-before campaign. A successful
        # append moves the thread past both without a separate tracking file.
        if (
            indexed
            and COST_SUFFIX.search(indexed[0])
            and len(indexed[0]) <= TITLE_LIMIT
            and indexed[1]
            and indexed[1] >= session_updated_at
            and (reprice_before is None or indexed[1] >= reprice_before)
        ):
            continue
        selected.append(row)
    return selected[:limit]


def root_thread_ids(records: dict) -> set[str]:
    return {thread_id for thread_id, (_, meta) in records.items() if rollout_type(meta) == "root"}


def existing_thread_name(row: sqlite3.Row, index_names: Dict[str, str]) -> Optional[str]:
    if row["history_mode"] == "paginated":
        return (row["name"] or "").strip() or None
    title = (row["title"] or "").strip()
    if title and title != (row["first_user_message"] or "").strip():
        return title
    return (index_names.get(row["id"]) or "").strip() or None


def synthetic_title(row: sqlite3.Row) -> str:
    return " ".join((row["first_user_message"] or row["title"] or "Untitled").split())


def cost_title(title: str, stats) -> str:
    cost = stats.known_cost_usd
    incomplete = bool(stats.unpriced_models or stats.unattributed_tokens)
    suffix = f" · ${cost:,.2f}{'+' if incomplete else ''}"
    base = COST_SUFFIX.sub("", title).rstrip()
    if len(base) + len(suffix) > TITLE_LIMIT:
        base = base[: TITLE_LIMIT - len(suffix) - 1].rstrip() + "…"
    return base + suffix


def display_title(title: str) -> str:
    return " ".join("".join(character if character.isprintable() else " " for character in title).split())[:120]


def append_index(codex_home: Path, updates: List[Tuple[str, str]]) -> None:
    now = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    payload = "".join(
        json.dumps({"id": thread_id, "thread_name": title, "updated_at": now}) + "\n"
        for thread_id, title in updates
    ).encode()
    with (codex_home / "session_index.jsonl").open("a+b") as index:
        index.seek(0, os.SEEK_END)
        # A prior interrupted append may lack a newline; separate it so this record parses.
        if index.tell():
            index.seek(-1, os.SEEK_END)
            if index.read(1) != b"\n":
                index.write(b"\n")
        index.write(payload)
        index.flush()
        os.fsync(index.fileno())


@contextmanager
def updater_lock(codex_home: Path):
    lock_path = codex_home / "thread-cost-title-updater.lock"
    with lock_path.open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise SystemExit("another title updater is already running")
        yield


def update_titles(
    codex_home: Path,
    thread_ids: List[str],
    title_matches: List[str],
    idle_minutes: Optional[int],
    limit: int,
    max_runtime_seconds: Optional[int],
    reprice_before: Optional[datetime],
    apply: bool,
) -> List[Tuple[str, str, str]]:
    database = state_db_path(codex_home)
    deadline = time.monotonic() + max_runtime_seconds if max_runtime_seconds is not None else None
    with updater_lock(codex_home), sqlite3.connect(database) as connection:
        connection.row_factory = sqlite3.Row
        index_entries = latest_index_entries(codex_home)
        index_names = {thread_id: name for thread_id, (name, _) in index_entries.items()}
        # Build one snapshot per run; report() would rebuild it for every title.
        records, children = build_index(codex_home)
        roots = root_thread_ids(records)
        rows = (
            selected_threads(connection, index_names, roots, thread_ids, title_matches)
            if thread_ids or title_matches
            else idle_root_threads(
                connection, index_entries, roots, idle_minutes or 0,
                limit, reprice_before,
            )
        )
        updates = []
        for row in rows:
            if deadline is not None and time.monotonic() >= deadline:
                break
            old = existing_thread_name(row, index_names) or synthetic_title(row)
            new = cost_title(old, tree_stats(row["id"], records, children))
            updates.append((row["id"], old, new))
        if apply:
            connection.executemany(
                "UPDATE threads SET title = ?, name = ? WHERE id = ?",
                [(new, new, thread_id) for thread_id, _, new in updates],
            )
            connection.commit()
        if apply:
            append_index(codex_home, [(thread_id, new) for thread_id, _, new in updates])
    return updates


def self_test() -> None:
    stats = type(
        "Stats",
        (),
        {"known_cost_usd": 12.34, "unpriced_models": set(), "unattributed_tokens": 0},
    )()
    assert cost_title("Short title", stats) == "Short title · $12.34"
    long_title = cost_title(
        "Investigate the title updater specifically to determine why it failed", stats
    )
    assert len(long_title) <= 65
    assert long_title.endswith("… · $12.34")
    assert display_title("Title\x1b[31m\nforged") == "Title [31m forged"
    with tempfile.TemporaryDirectory() as directory:
        home = Path(directory)
        database = home / "state_5.sqlite"
        now = int(datetime.now(timezone.utc).timestamp())
        with sqlite3.connect(database) as connection:
            connection.execute(
                "CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT NOT NULL, name TEXT, history_mode TEXT NOT NULL, archived INTEGER NOT NULL, updated_at INTEGER NOT NULL, first_user_message TEXT)"
            )
            connection.executemany(
                "INSERT INTO threads VALUES (?, ?, ?, 'legacy', 0, ?, ?)",
                [
                    ("already", "Already · $1.00", "Already · $1.00", now - 16 * 60, "Prompt"),
                    (
                        "overlong",
                        "A" * 70 + " · $1.00",
                        "A" * 70 + " · $1.00",
                        now - 17 * 60,
                        "Prompt",
                    ),
                    ("stale", "Stale", None, now - 18 * 60, "Prompt"),
                    ("child", "Internal child", None, now - 19 * 60, "Prompt"),
                ],
            )
        sessions = home / "sessions"
        sessions.mkdir()
        (sessions / "root.jsonl").write_text(
            json.dumps({"type": "session_meta", "payload": {"id": "stale", "source": "cli"}}) + "\n"
        )
        (sessions / "child.jsonl").write_text(
            json.dumps(
                {
                    "type": "session_meta",
                    "payload": {
                        "id": "child",
                        "parent_thread_id": "stale",
                        "source": {"subagent": {"thread_spawn": {"parent_thread_id": "stale"}}},
                    },
                }
            )
            + "\n"
        )
        old_timestamp = datetime.fromtimestamp(now - 5 * 60, timezone.utc)
        timestamp = old_timestamp.isoformat().replace("+00:00", "Z")
        (home / "session_index.jsonl").write_text(
            json.dumps({"id": "already", "thread_name": "Already · $1.00", "updated_at": timestamp}) + "\n"
            + json.dumps({"id": "overlong", "thread_name": "A" * 70 + " · $1.00", "updated_at": timestamp}) + "\n"
        )
        with sqlite3.connect(database) as connection:
            connection.row_factory = sqlite3.Row
            assert [
                row["id"]
                for row in idle_root_threads(
                    connection,
                    latest_index_entries(home),
                    {"already", "overlong", "stale"},
                    15,
                    10,
                )
            ] == ["overlong", "stale"]
            cutoff = datetime.fromtimestamp(now - 60, timezone.utc)
            assert [row["id"] for row in idle_root_threads(connection, latest_index_entries(home), {"already", "stale"}, 15, 1, cutoff)] == ["already"]
        (home / "session_index.jsonl").write_bytes(b'{"incomplete":')
        append_index(home, [("stale", "Stale · $1.23")])
        assert latest_index_names(home)["stale"] == "Stale · $1.23"
        try:
            update_titles(home, ["child"], [], None, 20, None, None, False)
        except SystemExit as error:
            assert "root" in str(error)
        else:
            raise AssertionError("explicit selection accepted a non-root task")
    print("self-test passed")


def parse_max_runtime(value: str) -> int:
    match = re.fullmatch(r"([1-9]\d*)(m)?", value)
    if not match:
        raise argparse.ArgumentTypeError("must be bare seconds or whole minutes with an m suffix")
    return int(match.group(1)) * (60 if match.group(2) else 1)


def parse_reprice_before(value: str) -> datetime:
    if re.fullmatch(r"\d{4}-\d{2}-\d{2}", value):
        value += "T00:00:00Z"
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        raise argparse.ArgumentTypeError("must be an ISO date or timestamp") from None
    if parsed.tzinfo is None:
        raise argparse.ArgumentTypeError("timestamp must include a time zone")
    return parsed.astimezone(timezone.utc)


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--thread-id", action="append", default=[], help="exact thread id; repeatable")
    parser.add_argument("--match-title", action="append", default=[], help="unique case-insensitive title substring; repeatable")
    parser.add_argument("--idle-minutes", type=int, help="select root sessions idle for at least this many minutes")
    parser.add_argument("--limit", type=int, default=20, help="maximum idle sessions per run (default: 20)")
    parser.add_argument(
        "--max-runtime",
        type=parse_max_runtime,
        help="stop before starting another session after SECONDS or whole MINUTESm (for example: 90 or 5m)",
    )
    parser.add_argument(
        "--reprice-before",
        type=parse_reprice_before,
        help="reprocess cost titles last written before this fixed ISO date or timestamp",
    )
    parser.add_argument(
        "--install-launch-agent",
        action="store_true",
        help="install and load the five-minute idle updater LaunchAgent",
    )
    parser.add_argument("--apply", action="store_true", help="write the SQLite and session-index updates")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument(
        "--codex-home",
        type=Path,
        default=Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")),
    )
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    home = args.codex_home.expanduser()
    if args.install_launch_agent:
        if args.thread_id or args.match_title or args.idle_minutes is not None or args.reprice_before or args.apply:
            parser.error("--install-launch-agent cannot be combined with update options")
        print(f"installed and loaded {install_launch_agent(home)}")
        return
    if not (args.thread_id or args.match_title or args.idle_minutes is not None):
        parser.print_help()
        return
    if bool(args.thread_id or args.match_title) == (args.idle_minutes is not None):
        parser.error("provide --thread-id/--match-title or --idle-minutes, but not both")
    if args.idle_minutes is not None and (args.idle_minutes < 1 or args.limit < 1):
        parser.error("idle selection values must be positive")
    if args.reprice_before is not None and args.reprice_before > datetime.now(timezone.utc):
        parser.error("--reprice-before cannot be in the future")
    if args.reprice_before is not None and args.idle_minutes is None:
        parser.error("--reprice-before requires --idle-minutes")
    updates = update_titles(
        home, args.thread_id, args.match_title, args.idle_minutes, args.limit,
        args.max_runtime, args.reprice_before, args.apply,
    )
    for thread_id, old, new in updates:
        print(f"{display_title(thread_id)}: {display_title(old)} -> {display_title(new)}")
    print("updated" if args.apply else "dry run; pass --apply to write")


if __name__ == "__main__":
    main()
