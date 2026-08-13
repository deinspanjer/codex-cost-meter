#!/usr/bin/env python3
"""Summarize a Codex rollout and all linked descendant rollout files.

Costs use the model and timestamp of each token event. Single-metadata legacy
rollouts can be costed without turn IDs; ambiguous embedded history is omitted
and reported as unattributed rather than guessed.
"""

from __future__ import annotations

import argparse
import json
import os
import tempfile
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path


PRICING_AS_OF = "2026-08-06"
PRICING_SOURCE = "https://developers.openai.com/api/docs/pricing"

# Standard short-context API prices per 1M tokens: input, cached input,
# cache write, output. Sources: developers.openai.com, pydantic/genai-prices,
# and BerriAI/litellm. The first row is the earliest date that rate is known;
# add another row only for a known price change. None means no published rate.
PRICE_HISTORY = {
    "gpt-5.1": [("2025-11-13", (1.25, 0.125, None, 10.0))],
    "gpt-5.1-codex": [("2025-11-13", (1.25, 0.125, None, 10.0))],
    "gpt-5.1-codex-mini": [("2025-11-13", (0.25, 0.025, None, 2.0))],
    "gpt-5.1-codex-max": [("2025-11-13", (1.25, 0.125, None, 10.0))],
    "gpt-5.2": [("2025-12-11", (1.75, 0.175, None, 14.0))],
    "gpt-5.2-pro": [("2025-12-11", (21.0, None, None, 168.0))],
    "gpt-5.2-codex": [("2025-12-11", (1.75, 0.175, None, 14.0))],
    "gpt-5.3-codex": [("2026-02-05", (1.75, 0.175, None, 14.0))],
    "gpt-5.4": [("2026-03-05", (2.5, 0.25, None, 15.0))],
    "gpt-5.4-pro": [("2026-03-05", (30.0, None, None, 180.0))],
    "gpt-5.4-mini": [("2026-03-17", (0.75, 0.075, None, 4.5))],
    "gpt-5.4-nano": [("2026-03-17", (0.2, 0.02, None, 1.25))],
    "gpt-5.5": [("2026-04-23", (5.0, 0.5, None, 30.0))],
    "gpt-5.5-pro": [("2026-04-23", (30.0, None, None, 180.0))],
    "gpt-5.6-sol": [("2026-07-09", (5.0, 0.5, 6.25, 30.0))],
    "gpt-5.6-terra": [
        ("2026-07-09", (2.5, 0.25, 3.125, 15.0)),
        ("2026-07-30", (2.0, 0.2, 2.5, 12.0)),
    ],
    "gpt-5.6-luna": [
        ("2026-07-09", (1.0, 0.1, 1.25, 6.0)),
        ("2026-07-30", (0.2, 0.02, 0.25, 1.2)),
    ],
}
PRICE_PROXIES = {
    "gpt-5.6": "gpt-5.6-sol",
    "codex-auto-review": "gpt-5.6-luna",
}
UNDATED_PROXIES = {"codex-auto-review"}

TOKEN_FIELDS = (
    "input_tokens",
    "cached_input_tokens",
    "cache_write_input_tokens",
    "output_tokens",
    "reasoning_output_tokens",
)
START_EVENTS = {"task_started", "turn_started"}
END_EVENTS = {"task_complete", "turn_complete", "task_aborted", "turn_aborted"}


@dataclass
class Stats:
    tokens: Counter = field(default_factory=Counter)
    model_tokens: dict[str, Counter] = field(default_factory=lambda: defaultdict(Counter))
    model_costs: Counter = field(default_factory=Counter)
    turn_models: Counter = field(default_factory=Counter)
    turns: int = 0
    ended_turns: int = 0
    duration_seconds: float = 0.0
    known_cost_usd: float = 0.0
    unpriced_models: Counter = field(default_factory=Counter)
    unattributed_tokens: int = 0
    malformed_lines: int = 0
    rollout_count: int = 1

    def add(self, other: "Stats") -> None:
        self.tokens.update(other.tokens)
        for model, tokens in other.model_tokens.items():
            self.model_tokens[model].update(tokens)
        self.model_costs.update(other.model_costs)
        self.turn_models.update(other.turn_models)
        self.turns += other.turns
        self.ended_turns += other.ended_turns
        self.duration_seconds += other.duration_seconds
        self.known_cost_usd += other.known_cost_usd
        self.unpriced_models.update(other.unpriced_models)
        self.unattributed_tokens += other.unattributed_tokens
        self.malformed_lines += other.malformed_lines
        self.rollout_count += other.rollout_count


def timestamp(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def first_meta(path: Path) -> dict | None:
    try:
        with path.open() as lines:
            for line in lines:
                try:
                    item = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if item.get("type") == "session_meta":
                    return item.get("payload", {})
    except OSError:
        return None
    return None


def rollout_id(meta: dict) -> str | None:
    return meta.get("id") or meta.get("session_id")


def parent_id(meta: dict) -> str | None:
    if meta.get("parent_thread_id"):
        return meta["parent_thread_id"]
    source = meta.get("source")
    if isinstance(source, dict):
        subagent = source.get("subagent")
        if isinstance(subagent, dict):
            spawn = subagent.get("thread_spawn")
            if isinstance(spawn, dict):
                return spawn.get("parent_thread_id")
    return None


def rollout_type(meta: dict) -> str:
    source = meta.get("source")
    if isinstance(source, dict) and "internal" in source:
        internal = source["internal"]
        if internal == "memory_consolidation":
            return "memory_consolidation"
        return f"internal:{internal}"
    if not isinstance(source, dict) or "subagent" not in source:
        return "root"

    subagent = source["subagent"]
    if isinstance(subagent, str):
        kind, label = subagent, None
    elif isinstance(subagent, dict) and subagent:
        kind, label = next(iter(subagent.items()))
    else:
        return "subagent"

    if kind == "thread_spawn":
        return "subagent"
    if kind == "review":
        return "code_review"
    if kind == "compact":
        return "compaction"
    if kind == "memory_consolidation":
        return "memory_consolidation"
    if kind == "other" and label == "guardian":
        return "security_review"
    return f"subagent:{label or kind}"


def build_index(codex_home: Path) -> tuple[dict, dict]:
    records = {}
    children = defaultdict(list)
    for root in (codex_home / "sessions", codex_home / "archived_sessions"):
        if not root.is_dir():
            continue
        for path in root.rglob("*.jsonl"):
            meta = first_meta(path)
            item_id = rollout_id(meta or {})
            if not item_id:
                continue
            previous = records.get(item_id)
            if previous is None or path.stat().st_mtime_ns > previous[0].stat().st_mtime_ns:
                records[item_id] = (path, meta)
    for item_id, (_, meta) in records.items():
        if parent := parent_id(meta):
            children[parent].append(item_id)
    return records, children


def descendants(root_id: str, children: dict) -> list[str]:
    found = []
    seen = {root_id}
    pending = list(children.get(root_id, ()))
    while pending:
        item_id = pending.pop()
        if item_id in seen:
            continue
        seen.add(item_id)
        found.append(item_id)
        pending.extend(children.get(item_id, ()))
    return found


def tree_stats(root_id: str, records: dict, children: dict) -> Stats:
    """Return aggregate stats using an index already built by the caller."""
    if root_id not in records:
        raise SystemExit(f"rollout not found: {root_id}")
    total = Stats(rollout_count=0)
    for item_id in [root_id, *descendants(root_id, children)]:
        total.add(analyze_rollout(records[item_id][0]))
    return total


def price_rates(model: str, rollout_at: datetime | None) -> tuple | None:
    original_model = model
    model = PRICE_PROXIES.get(model, model)
    history = PRICE_HISTORY.get(model)
    if not history:
        return None
    if rollout_at is None or original_model in UNDATED_PROXIES:
        return history[-1][1]
    date = rollout_at.date().isoformat()
    return next((rates for effective_from, rates in reversed(history) if effective_from <= date), None)


def request_cost(model: str, usage: dict, rollout_at: datetime | None = None) -> float | None:
    rates = price_rates(model, rollout_at)
    if rates is None:
        return None
    input_tokens = max(0, int(usage.get("input_tokens", 0)))
    cached = max(0, int(usage.get("cached_input_tokens", 0)))
    cache_write = max(0, int(usage.get("cache_write_input_tokens", 0)))
    uncached = max(0, input_tokens - cached - cache_write)
    counts = (uncached, cached, cache_write, max(0, int(usage.get("output_tokens", 0))))
    if any(count and rate is None for count, rate in zip(counts, rates)):
        return None
    return sum(count * (rate or 0.0) for count, rate in zip(counts, rates)) / 1_000_000


def analyze_rollout(path: Path) -> Stats:
    stats = Stats()
    valid_turns = set()
    session_meta_count = 0
    rollout_at = None
    with path.open() as lines:
        for line in lines:
            try:
                item = json.loads(line)
            except json.JSONDecodeError:
                continue
            if item.get("type") == "session_meta":
                session_meta_count += 1
                if item.get("timestamp"):
                    try:
                        rollout_at = timestamp(item["timestamp"])
                    except (KeyError, ValueError):
                        pass
            if item.get("type") == "turn_context" and item.get("payload", {}).get("turn_id"):
                valid_turns.add(item["payload"]["turn_id"])

    starts = {}
    turn_ids = []
    turn_model = {}
    active_turn = None
    active_model = None
    legacy_model = None
    previous_total_usage = None

    with path.open() as lines:
        for line in lines:
            try:
                item = json.loads(line)
            except json.JSONDecodeError:
                stats.malformed_lines += 1
                continue
            payload = item.get("payload", {})
            item_type = item.get("type")
            payload_type = payload.get("type")

            if item_type == "turn_context":
                turn_id = payload.get("turn_id")
                model = payload.get("model") or "unknown"
                effort = payload.get("effort") or payload.get("reasoning_effort") or "unknown"
                if turn_id:
                    legacy_model = None
                    turn_model[turn_id] = (model, effort)
                    if turn_id == active_turn:
                        active_model = model
                elif session_meta_count == 1:
                    # Old rollouts have model-bearing contexts but no turn IDs.
                    # Multiple metadata records indicate embedded history, so
                    # only the unambiguous single-metadata form gets this fallback.
                    legacy_model = model
                continue

            if item_type != "event_msg":
                continue
            if payload_type in START_EVENTS:
                turn_id = payload.get("turn_id") or payload.get("id")
                active_turn = turn_id if turn_id in valid_turns else None
                active_model = turn_model.get(active_turn, (None, None))[0]
                if active_turn:
                    legacy_model = None
                    starts[active_turn] = timestamp(item["timestamp"])
                    turn_ids.append(active_turn)
                continue
            if payload_type in END_EVENTS:
                turn_id = payload.get("turn_id") or payload.get("id") or active_turn
                if turn_id in starts:
                    stats.duration_seconds += (timestamp(item["timestamp"]) - starts.pop(turn_id)).total_seconds()
                    stats.ended_turns += 1
                if turn_id == active_turn:
                    active_turn = active_model = None
                if turn_id is None:
                    legacy_model = None
                continue
            if payload_type != "token_count":
                continue

            info = payload.get("info") or {}
            last_usage = info.get("last_token_usage")
            total_usage = info.get("total_token_usage")
            if not last_usage:
                continue
            if total_usage and previous_total_usage:
                reset = any(
                    int(total_usage.get(name, 0)) < int(previous_total_usage.get(name, 0))
                    for name in (*TOKEN_FIELDS, "total_tokens")
                )
                usage = last_usage if reset else {
                    name: max(0, int(total_usage.get(name, 0)) - int(previous_total_usage.get(name, 0)))
                    for name in (*TOKEN_FIELDS, "total_tokens")
                }
            else:
                usage = last_usage
            if total_usage:
                previous_total_usage = total_usage
            if not any(int(usage.get(name, 0)) for name in TOKEN_FIELDS):
                continue
            if active_turn is None and legacy_model is None:
                if not valid_turns and session_meta_count > 1:
                    # Preserve the incomplete-cost signal without double-charging
                    # history copied into resumed or forked legacy rollouts.
                    stats.unattributed_tokens += max(0, int(usage.get("total_tokens", 0)))
                continue
            for name in TOKEN_FIELDS:
                stats.tokens[name] += max(0, int(usage.get(name, 0)))
            model = legacy_model if active_turn is None else (
                active_model or turn_model.get(active_turn, ("unknown", "unknown"))[0]
            )
            stats.model_tokens[model].update(
                {name: max(0, int(usage.get(name, 0))) for name in TOKEN_FIELDS}
            )
            try:
                request_at = timestamp(item["timestamp"])
            except (KeyError, ValueError):
                request_at = rollout_at
            cost = request_cost(model, usage, request_at)
            if cost is None:
                stats.unpriced_models[model] += max(0, int(usage.get("total_tokens", 0)))
            else:
                stats.known_cost_usd += cost
                stats.model_costs[model] += cost

    stats.turns = len(turn_ids)
    stats.turn_models.update(turn_model.get(turn_id, ("unknown", "unknown")) for turn_id in turn_ids)
    return stats


def majority(counter: Counter) -> tuple[str | None, str | None]:
    if not counter:
        return None, None
    return sorted(counter, key=lambda key: (-counter[key], key))[0]


def public_stats(stats: Stats, include_rollout_count: bool = False) -> dict:
    model, effort = majority(stats.turn_models)
    result = {
        "majority_turn_model": model,
        "majority_reasoning_level": effort,
        "input_tokens": stats.tokens["input_tokens"],
        "input_cache_write_tokens": stats.tokens["cache_write_input_tokens"],
        "input_cache_read_tokens": stats.tokens["cached_input_tokens"],
        "reasoning_tokens": stats.tokens["reasoning_output_tokens"],
        "output_tokens": stats.tokens["output_tokens"],
        "turns": stats.turns,
        "completed_or_aborted_turns": stats.ended_turns,
        "incomplete_turns": max(0, stats.turns - stats.ended_turns),
        "total_turn_duration_seconds": round(stats.duration_seconds, 3),
        "estimated_cost_usd": None if stats.unpriced_models or stats.unattributed_tokens else round(stats.known_cost_usd, 8),
        "known_model_cost_usd": round(stats.known_cost_usd, 8),
        "unpriced_models": dict(sorted(stats.unpriced_models.items())),
    }
    if stats.unattributed_tokens:
        result["unattributed_usage_tokens"] = stats.unattributed_tokens
    if include_rollout_count:
        result = {"rollout_count": stats.rollout_count, **result}
    if stats.malformed_lines:
        result["malformed_lines_skipped"] = stats.malformed_lines
    return result


def public_model_stats(stats: Stats) -> dict:
    models = set(stats.model_tokens) | set(stats.model_costs) | {
        model for model, _ in stats.turn_models
    }
    return {
        model: {
            "turns": sum(count for (turn_model, _), count in stats.turn_models.items() if turn_model == model),
            "input_tokens": stats.model_tokens[model]["input_tokens"],
            "input_cache_write_tokens": stats.model_tokens[model]["cache_write_input_tokens"],
            "input_cache_read_tokens": stats.model_tokens[model]["cached_input_tokens"],
            "reasoning_tokens": stats.model_tokens[model]["reasoning_output_tokens"],
            "output_tokens": stats.model_tokens[model]["output_tokens"],
            "estimated_cost_usd": (
                None if model in stats.unpriced_models else round(stats.model_costs[model], 8)
            ),
            "known_model_cost_usd": round(stats.model_costs[model], 8),
        }
        for model in sorted(models)
    }


def latest_thread_name(codex_home: Path, root_id: str) -> str | None:
    latest = ("", None)
    try:
        with (codex_home / "session_index.jsonl").open() as lines:
            for line in lines:
                try:
                    item = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if item.get("id") == root_id and item.get("thread_name"):
                    candidate = (item.get("updated_at") or "", item["thread_name"])
                    if candidate[0] >= latest[0]:
                        latest = candidate
    except OSError:
        pass
    return latest[1]


def report(root_id: str, codex_home: Path) -> dict:
    records, children = build_index(codex_home)
    if root_id not in records:
        raise SystemExit(f"rollout not found under {codex_home}: {root_id}")

    child_ids = descendants(root_id, children)
    ids = [root_id, *child_ids]
    analyzed = {item_id: analyze_rollout(records[item_id][0]) for item_id in ids}

    tree = Stats(rollout_count=0)
    child_stats = Stats(rollout_count=0)
    by_type = {}
    for item_id in ids:
        stats = analyzed[item_id]
        tree.add(stats)
        if item_id != root_id:
            child_stats.add(stats)
        kind = rollout_type(records[item_id][1])
        by_type.setdefault(kind, Stats(rollout_count=0)).add(stats)

    root = public_stats(analyzed[root_id])
    root.update(
        {
            "rollout_id": root_id,
            "rollout_type": rollout_type(records[root_id][1]),
            "total_subagent_spawns": len(child_ids),
            "total_subagent_turn_duration_seconds": round(child_stats.duration_seconds, 3),
        }
    )
    if project := records[root_id][1].get("cwd"):
        root["project"] = project
    if name := latest_thread_name(codex_home, root_id):
        root["thread_name"] = name
    return {
        "rollout": root,
        "tree": public_stats(tree, include_rollout_count=True),
        "by_model": public_model_stats(tree),
        "by_rollout_type": {
            kind: public_stats(stats, include_rollout_count=True)
            for kind, stats in sorted(by_type.items())
        },
        "pricing": {
            "basis": "standard API list pricing; per request/turn model; output includes reasoning",
            "as_of": PRICING_AS_OF,
            "source": PRICING_SOURCE,
            "model_proxies": PRICE_PROXIES,
        },
    }


def human_number(value: int) -> str:
    for size, suffix in ((1_000_000_000, "B"), (1_000_000, "M"), (1_000, "K")):
        if value >= size:
            return f"{value / size:.3g}{suffix}"
    return str(value)


def human_duration(seconds: float) -> str:
    seconds = round(seconds)
    if seconds < 60:
        return f"{seconds}s"
    hours, remainder = divmod(seconds, 3600)
    minutes = remainder // 60
    return f"{hours}h {minutes:02d}m" if hours else f"{minutes}m"


def human_cost(stats: dict) -> str:
    cost = stats["known_model_cost_usd"]
    return f"${cost:,.2f}{'+' if stats.get('unpriced_models') or stats.get('estimated_cost_usd') is None else ''}"


def safe_text(value: object) -> str:
    return "".join(character if character.isprintable() else " " for character in str(value))


def text_table(headers: list[str], rows: list[list[str]], right: set[int]) -> str:
    widths = [max(len(row[index]) for row in [headers, *rows]) for index in range(len(headers))]
    lines = []
    for row in [headers, *rows]:
        lines.append(
            "  ".join(
                value.rjust(widths[index]) if index in right else value.ljust(widths[index])
                for index, value in enumerate(row)
            ).rstrip()
        )
    return "\n".join(lines)


def human_report(result: dict) -> str:
    rollout = result["rollout"]
    tree = result["tree"]
    primary = f"{rollout['majority_turn_model']} / {rollout['majority_reasoning_level']}"
    incomplete = tree["incomplete_turns"]
    turn_summary = f"{tree['turns']} ({tree['completed_or_aborted_turns']} ended"
    turn_summary += f", {incomplete} incomplete" if incomplete else ""
    turn_summary += ")"

    scope_rows = [[
        "Own rollout",
        "1",
        str(rollout["turns"]),
        human_duration(rollout["total_turn_duration_seconds"]),
        human_cost(rollout),
    ]]
    labels = {
        "subagent": "Spawned subagents",
        "security_review": "Security reviews",
        "code_review": "Code reviews",
        "compaction": "Compactions",
        "memory_consolidation": "Memory consolidation",
    }
    root_type = rollout["rollout_type"]
    for kind, stats in result["by_rollout_type"].items():
        if kind == root_type:
            continue
        scope_rows.append([
            safe_text(labels.get(kind, kind.replace("_", " ").title())),
            str(stats["rollout_count"]),
            str(stats["turns"]),
            human_duration(stats["total_turn_duration_seconds"]),
            human_cost(stats),
        ])
    scope_rows.append([
        "Whole tree",
        str(tree["rollout_count"]),
        str(tree["turns"]),
        human_duration(tree["total_turn_duration_seconds"]),
        human_cost(tree),
    ])

    include_writes = tree["input_cache_write_tokens"] > 0
    token_headers = ["Model", "Input", "Cache read"]
    if include_writes:
        token_headers.append("Cache write")
    token_headers.extend(["Output", "Reasoning", "Est. cost"])
    model_rows = []
    by_model = result["by_model"]
    for model, stats in sorted(
        by_model.items(), key=lambda item: (-item[1]["known_model_cost_usd"], item[0])
    ):
        row = [
            safe_text(model),
            human_number(stats["input_tokens"]),
            human_number(stats["input_cache_read_tokens"]),
        ]
        if include_writes:
            row.append(human_number(stats["input_cache_write_tokens"]))
        row.extend([
            human_number(stats["output_tokens"]),
            human_number(stats["reasoning_tokens"]),
            human_cost(stats),
        ])
        model_rows.append(row)
    total_row = [
        "Total",
        human_number(tree["input_tokens"]),
        human_number(tree["input_cache_read_tokens"]),
    ]
    if include_writes:
        total_row.append(human_number(tree["input_cache_write_tokens"]))
    total_row.extend([
        human_number(tree["output_tokens"]),
        human_number(tree["reasoning_tokens"]),
        human_cost(tree),
    ])
    model_rows.append(total_row)

    notes = [
        "Cache read is included in input. Reasoning is included in output tokens and output cost.",
        "Agent time sums turn durations and may overlap when rollouts run concurrently.",
    ]
    if tree.get("unattributed_usage_tokens"):
        notes.append("Some legacy usage could not be separated safely from embedded history.")
    used_proxies = {
        model: proxy for model, proxy in result["pricing"]["model_proxies"].items() if model in by_model
    }
    if used_proxies:
        notes.insert(1, "; ".join(f"{safe_text(model)} is priced using {safe_text(proxy)} as a proxy" for model, proxy in used_proxies.items()) + ".")
    notes.append(
        f"Pricing: standard API list pricing as of {result['pricing']['as_of']}."
    )
    header = [
        f"Codex rollout {safe_text(rollout['rollout_id'])}",
        *([f"Project: {safe_text(rollout['project'])}"] if rollout.get("project") else []),
        *([f"Name: {safe_text(rollout['thread_name'])}"] if rollout.get("thread_name") else []),
        f"Type: {safe_text(rollout['rollout_type'])}   Primary: {safe_text(primary)}   Descendants: {rollout['total_subagent_spawns']}",
        f"Turns: {turn_summary}",
    ]
    return "\n".join([
        *header,
        "",
        text_table(["Scope", "Rollouts", "Turns", "Agent time", "Est. cost"], scope_rows, {1, 2, 3, 4}),
        "",
        "Tokens and estimated cost by model",
        text_table(token_headers, model_rows, set(range(1, len(token_headers)))),
        "",
        *notes,
    ])


def self_test() -> None:
    def write(path: Path, rows: list[dict]) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("".join(json.dumps(row) + "\n" for row in rows))

    with tempfile.TemporaryDirectory() as directory:
        home = Path(directory)
        root_id, child_id = "root", "child"
        root_rows = [
            {"timestamp": "2026-07-29T23:59:50Z", "type": "session_meta", "payload": {"id": root_id, "source": "vscode", "cwd": "/tmp/project"}},
            {"timestamp": "2026-07-30T00:00:00Z", "type": "event_msg", "payload": {"type": "task_started", "turn_id": "t1"}},
            {"timestamp": "2026-07-30T00:00:01Z", "type": "turn_context", "payload": {"turn_id": "t1", "model": "gpt-5.6-terra", "effort": "high"}},
            {"timestamp": "2026-07-30T00:00:05Z", "type": "event_msg", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "cached_input_tokens": 20, "cache_write_input_tokens": 10, "output_tokens": 5, "reasoning_output_tokens": 3, "total_tokens": 105}}}},
            {"timestamp": "2026-07-30T00:00:10Z", "type": "event_msg", "payload": {"type": "task_complete", "turn_id": "t1"}},
        ]
        child_rows = [
            {"timestamp": "2026-07-30T00:00:02Z", "type": "session_meta", "payload": {"id": child_id, "parent_thread_id": root_id, "source": {"subagent": {"other": "guardian"}}}},
            {"timestamp": "2026-07-30T00:00:02Z", "type": "event_msg", "payload": {"type": "task_started", "turn_id": "c1"}},
            {"timestamp": "2026-07-30T00:00:03Z", "type": "turn_context", "payload": {"turn_id": "c1", "model": "codex-auto-review", "effort": "low"}},
            {"timestamp": "2026-07-30T00:00:05Z", "type": "event_msg", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "cached_input_tokens": 20, "cache_write_input_tokens": 10, "output_tokens": 5, "reasoning_output_tokens": 3, "total_tokens": 105}}}},
            {"timestamp": "2026-07-30T00:00:06Z", "type": "event_msg", "payload": {"type": "task_complete", "turn_id": "c1"}},
        ]
        write(home / "sessions/2026/07/29/root.jsonl", root_rows)
        write(home / "archived_sessions/child.jsonl", child_rows)
        write(home / "session_index.jsonl", [
            {"id": root_id, "thread_name": "Old name", "updated_at": "2026-07-29T00:00:00Z"},
            {"id": root_id, "thread_name": "Rollout stats", "updated_at": "2026-07-30T00:00:00Z"},
        ])
        result = report(root_id, home)
        records, children = build_index(home)
        assert round(tree_stats(root_id, records, children).known_cost_usd, 8) == result["tree"]["known_model_cost_usd"]
        assert result["rollout"]["project"] == "/tmp/project"
        assert result["rollout"]["thread_name"] == "Rollout stats"
        assert result["rollout"]["turns"] == 1
        assert result["rollout"]["input_cache_read_tokens"] == 20
        assert result["rollout"]["total_turn_duration_seconds"] == 10.0
        assert result["rollout"]["total_subagent_spawns"] == 1
        assert result["rollout"]["total_subagent_turn_duration_seconds"] == 4.0
        assert result["by_rollout_type"]["security_review"]["rollout_count"] == 1
        assert result["by_rollout_type"]["security_review"]["estimated_cost_usd"] is not None
        assert result["by_model"]["gpt-5.6-terra"]["input_tokens"] == 100
        assert result["by_model"]["gpt-5.6-terra"]["known_model_cost_usd"] == 0.000229
        assert result["by_model"]["codex-auto-review"]["input_cache_write_tokens"] == 10
        assert "Tokens and estimated cost by model" in human_report(result)
        assert "Project: /tmp/project\nName: Rollout stats" in human_report(result)
        assert "Name: Rollout stats" in human_report(result)
        assert "Cache write" in human_report(result)
        assert result["pricing"]["model_proxies"]["codex-auto-review"] == "gpt-5.6-luna"
        assert result["rollout"]["estimated_cost_usd"] is not None
        assert request_cost("gpt-5.2-codex", {"input_tokens": 1_000_000}) == 1.75
        assert request_cost("gpt-5.6-terra", {"output_tokens": 1_000_000}, timestamp("2026-07-29T00:00:00Z")) == 15.0
        assert request_cost("gpt-5.6-terra", {"output_tokens": 1_000_000}, timestamp("2026-07-30T00:00:00Z")) == 12.0
        assert price_rates("gpt-5.4-codex", timestamp("2026-03-13T00:00:00Z")) is None
        result["rollout"]["project"] = "/tmp/project\x1b[31m\nforged"
        assert "\x1b" not in human_report(result)

        legacy = home / "legacy.jsonl"
        write(legacy, [
            {"timestamp": "2026-01-20T00:00:00Z", "type": "session_meta", "payload": {"id": "legacy", "source": "cli"}},
            {"timestamp": "2026-01-20T00:00:01Z", "type": "turn_context", "payload": {"model": "gpt-5.2-codex"}},
            {"timestamp": "2026-01-20T00:00:02Z", "type": "event_msg", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "cached_input_tokens": 20, "output_tokens": 5, "reasoning_output_tokens": 3, "total_tokens": 105}}}},
        ])
        legacy_stats = analyze_rollout(legacy)
        assert legacy_stats.tokens["input_tokens"] == 100
        assert legacy_stats.known_cost_usd > 0
        assert legacy_stats.turns == 0

        embedded = home / "embedded.jsonl"
        write(embedded, [
            {"timestamp": "2026-01-20T00:00:00Z", "type": "session_meta", "payload": {"id": "outer", "source": "cli"}},
            {"timestamp": "2026-01-20T00:00:00Z", "type": "session_meta", "payload": {"id": "copied", "source": "cli"}},
            {"timestamp": "2026-01-20T00:00:01Z", "type": "turn_context", "payload": {"model": "gpt-5.2-codex"}},
            {"timestamp": "2026-01-20T00:00:02Z", "type": "event_msg", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "cached_input_tokens": 20, "output_tokens": 5, "reasoning_output_tokens": 3, "total_tokens": 105}}}},
        ])
        embedded_stats = analyze_rollout(embedded)
        assert embedded_stats.known_cost_usd == 0
        assert embedded_stats.unattributed_tokens == 105
    print("self-test passed")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rollout_id", nargs="?")
    parser.add_argument(
        "--codex-home",
        type=Path,
        default=Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")),
        help="Codex state directory (default: CODEX_HOME or ~/.codex)",
    )
    parser.add_argument("--json", action="store_true", help="print exact machine-readable JSON")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if not args.rollout_id:
        parser.error("rollout_id is required unless --self-test is used")
    result = report(args.rollout_id, args.codex_home.expanduser())
    print(json.dumps(result, indent=2) if args.json else human_report(result))


if __name__ == "__main__":
    main()
