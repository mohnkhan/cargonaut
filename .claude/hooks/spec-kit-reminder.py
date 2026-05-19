#!/usr/bin/env python3
"""Spec-kit workflow reminder — UserPromptSubmit hook for MyOS2026.

Fires on every prompt and injects a one-time reminder when the prompt looks
like feature/spec work but isn't a direct /speckit-* invocation. The reminder
nudges Claude to follow the spec-kit ladder (specify → clarify → plan → tasks
→ analyze → implement) per CONTRIBUTING.MD § Spec-kit pattern.

Why this exists: prior Claude sessions have jumped from a hand-rolled spec.md
straight to implementation, bypassing clarify/plan/tasks/analyze. CLAUDE.md
notes alone don't enforce this; a UserPromptSubmit hook does.

Output: prints a JSON object to stdout with hookSpecificOutput.additionalContext,
which the harness injects into Claude's view as a system-reminder block.

Fail-open semantics: any parse error or unexpected input → exit 0 silently
(don't break the user's prompt over a hook bug).
"""

import json
import re
import sys

REMINDER = """[spec-kit-reminder]
This prompt looks like feature/spec work. Per CONTRIBUTING.MD § Spec-kit pattern, the workflow ladder is:

  1. /speckit-specify    — write spec.md from template
  2. /speckit-clarify    — resolve ambiguities (≤5 questions)
  3. /speckit-plan       — Constitution check + research.md + data-model.md + contracts/ + quickstart.md
  4. /speckit-tasks      — dependency-ordered tasks.md
  5. /speckit-analyze    — cross-artifact consistency check
  6. /speckit-implement  — execute tasks.md

Use the full ladder even for half-day features — phases are short when the scope is small. Do NOT jump straight to coding from an existing spec.md.

Skip this reminder if the task is a true one-line fix, a docs-only PR, or repo infrastructure that does NOT get an NNN- branch."""


# Detect feature/spec work intent. Match common phrasings the project uses:
#   "start Tier 2"            → tier pick
#   "start #67" / "pick #67"  → issue pick
#   "implement #68"           → do the work
#   "build Feature 061"       → named feature
#   "add a new feature ..."   → open-ended
#   "spec out X"              → front-load
#   "work on issue #N"        → variant phrasing
TRIGGER_PATTERNS = [
    r"\b(implement|start|pick|do|build|add|work\s+on)\s+#?\d+\b",
    r"\b(implement|start|pick|do|build|add|work\s+on)\s+[Tt]ier[\s-]*\d+",
    r"\b(implement|start|pick|do|build|add|work\s+on)\s+[Ff]eature(\s+\d+)?",
    r"\b(implement|start|pick|do|build|add)\s+(a\s+(new\s+)?|the\s+(next|new)\s+)?[Ff]eature\b",
    r"\b(implement|start|pick|do|build|add|work\s+on)\s+issue\s+#?\d+",
    r"\bspec\s+out\b",
    r"\b(start|pick|do)\s+(this|that)\s+(feature|issue|tier)",
]


def main() -> int:
    try:
        data = json.load(sys.stdin)
    except Exception:
        return 0  # malformed input — fail open

    prompt = (data.get("prompt") or "").strip()
    if not prompt:
        return 0

    # Skip: the user is already invoking a speckit command directly.
    if re.search(r"(^|\s)/speckit-", prompt):
        return 0

    if not any(re.search(p, prompt, re.IGNORECASE) for p in TRIGGER_PATTERNS):
        return 0

    out = {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": REMINDER,
        }
    }
    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
