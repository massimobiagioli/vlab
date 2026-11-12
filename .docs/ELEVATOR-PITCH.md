# VLab — Elevator Pitch

vlab is a virtual AI-powered laboratory for turning ideas into working software.
Through a clean, Unix-style CLI, users create isolated labs, projects, and agile
Kanban boards populated by specialized AI agents. A simple human prompt—such as
“I need this feature”—is interpreted by a Tech Lead agent, transformed into a
structured user story with deliverables, priorities, and deadlines, and
delegated to the right agents. These agents then collaborate autonomously,
coordinate their work, and advance tasks across projects, while the user
remains in control through a small set of precise, composable commands. vlab
brings the structure of a software team into a lightweight, observable, and
scriptable development environment. Laboratories, projects, and agent profiles
can carry application-managed memories that the Tech Lead considers as bounded,
delimited context; memories are data and never grant capabilities or act as
instructions. A project may reference one GitHub repository; the GitHub key
remains a global settings-file secret, never project or database content.

## Status

This pitch states product intent only. It is binding only where it does not
contradict [`TECHNICAL-SPECIFICATIONS.md`](TECHNICAL-SPECIFICATIONS.md),
[`ARCHITECTURE.md`](ARCHITECTURE.md), or an accepted ADR. In particular,
autonomous agent collaboration is outside the initial release: the
specification limits model use to bounded Tech Lead planning and direct
questions.
