#!/usr/bin/env python3
"""Moves one app to a desktop-kit tag: the pin and the release notes.

    bump_app.py <app folder> <app name> <tag> [<kit folder>]

* Sets DESKTOP_KIT_REF to the tag in every workflow of the app that has it.
* Copies the kit's notes for that version (CHANGELOG.md here, `## [x.y.z]`)
  under the app's `## [Unreleased]`, `{app}` replaced by the app's name and
  rewrapped at 80 columns. A note the app already has is not added twice.

Cargo.lock is left to `cargo update -p desktop-kit`, run with the kit beside
the app. Prints `notes` when the app's CHANGELOG.md changed, `no-notes` when
it didn't (the PR then takes the `no-notes` label), and nothing more. Only
the standard library: it runs on a bare CI runner.
"""

import re
import sys
import textwrap
from pathlib import Path

GROUPS = ["Added", "Changed", "Fixed", "Removed"]


def pin(app: Path, tag: str) -> None:
    for wf in sorted((app / ".github" / "workflows").glob("*.y*ml")):
        text = wf.read_text()
        new = re.sub(
            r"^(\s*DESKTOP_KIT_REF:\s*)\S+", rf"\g<1>{tag}", text, flags=re.M
        )
        if new != text:
            wf.write_text(new)


def kit_notes(kit: Path, version: str) -> dict:
    """The kit's bullets for `version`, by group: {"Added": ["text", ...]}."""
    changelog = kit / "CHANGELOG.md"
    if not changelog.exists():
        return {}
    text = changelog.read_text()
    m = re.search(
        rf"^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## \[|\Z)", text, re.M | re.S
    )
    if not m:
        return {}
    notes, group = {}, None
    for line in m.group(1).splitlines():
        if line.startswith("### "):
            group = line[4:].strip()
        elif line.startswith("- ") and group:
            notes.setdefault(group, []).append(line[2:].strip())
        elif line.startswith("  ") and group and notes.get(group):
            notes[group][-1] += " " + line.strip()
    return notes


def flat(text: str) -> str:
    return " ".join(text.split())


def bullet(text: str) -> str:
    return textwrap.fill(
        text,
        width=80,
        initial_indent="- ",
        subsequent_indent="  ",
        break_on_hyphens=False,
        break_long_words=False,
    )


def add_notes(app: Path, name: str, notes: dict) -> bool:
    path = app / "CHANGELOG.md"
    if not notes or not path.exists():
        return False
    text = path.read_text()
    have = flat(text)
    wanted = {
        g: [n.replace("{app}", name) for n in ns]
        for g, ns in notes.items()
    }
    wanted = {g: [n for n in ns if flat(n) not in have] for g, ns in wanted.items()}
    wanted = {g: ns for g, ns in wanted.items() if ns}
    if not wanted:
        return False

    if "## [Unreleased]" not in text:
        first = re.search(r"^## \[", text, re.M)
        at = first.start() if first else len(text)
        text = text[:at] + "## [Unreleased]\n\n" + text[at:]

    start = text.index("## [Unreleased]")
    body_at = text.index("\n", start) + 1
    end_m = re.search(r"^## \[", text[body_at:], re.M)
    end = body_at + end_m.start() if end_m else len(text)
    section = text[body_at:end]

    for group in sorted(wanted, key=lambda g: GROUPS.index(g) if g in GROUPS else 99):
        block = "\n\n".join(bullet(n) for n in wanted[group])
        head = re.search(rf"^### {re.escape(group)}\n", section, re.M)
        if head:
            # After the group's last bullet: up to the next heading.
            nxt = re.search(r"^#{2,3} ", section[head.end():], re.M)
            at = head.end() + (nxt.start() if nxt else len(section) - head.end())
            before = section[:at].rstrip("\n")
            section = before + "\n\n" + block + "\n\n" + section[at:].lstrip("\n")
        else:
            # Before the first group that comes later in GROUPS, or at the end.
            later = [
                re.search(rf"^### {g}\n", section, re.M)
                for g in GROUPS[GROUPS.index(group) + 1 :]
                if group in GROUPS
            ]
            later = [m.start() for m in later if m]
            at = min(later) if later else len(section.rstrip("\n"))
            new = f"### {group}\n\n{block}\n\n"
            section = section[:at].rstrip("\n") + "\n\n" + new + section[at:].lstrip("\n")
    section = "\n" + section.strip("\n") + "\n\n"
    path.write_text(text[:body_at] + section + text[end:])
    return True


def main() -> None:
    if len(sys.argv) not in (4, 5):
        sys.exit(__doc__)
    app, name, tag = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
    kit = Path(sys.argv[4]) if len(sys.argv) == 5 else app.parent / "desktop-kit"
    pin(app, tag)
    changed = add_notes(app, name, kit_notes(kit, tag.lstrip("v")))
    print("notes" if changed else "no-notes")


if __name__ == "__main__":
    main()
