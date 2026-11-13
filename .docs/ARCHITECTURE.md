# Architecture

## Status

**Proposed baseline — not a description of verified implementation.**

The current implementation is a local Rust CLI with a bounded real execution
loop. The model has tools only through validated actions confined to a project
worktree (ADR-0050). The tool-oriented diagram below is the original
architectural baseline; the implemented scope is narrower.

The product domain, deployment environment, model provider, and persistence
requirements are not yet defined. These assumptions must be confirmed before
implementation decisions depend on them.

## Goals

- Deliver one useful end-to-end workflow before expanding capabilities.
- Keep execution bounded, observable, and recoverable.
- Separate model-generated proposals from permission to execute actions.
- Make failures and partial completion explicit.
- Favor safe, idiomatic Rust and minimal dependencies.

## Non-Goals

The initial architecture does not include:

- General multi-agent orchestration. The fixed, bounded single-agent execution
  in ADR-0050 is the only exception; it has no model-chosen control flow.
- Distributed execution.
- Autonomous permission escalation.
- Arbitrary shell execution outside the story worktree. Commands run with the
  story worktree as working directory, a bounded time, and capped output; there
  is no OS-level sandbox (ADR-0050).
- Autonomous memory decisions or model-chosen retention; scoped memories are application-managed, including a bounded planning summary, and remain untrusted context (ADR-0022, ADR-0036).
- A general-purpose plugin framework.
- Unrequested background activity.

Add these capabilities only when validated requirements justify them.

## Architectural Approach

Use a single process with a deterministic orchestration loop and, where
necessary, one agent.

The application owns control flow, permissions, execution budgets, and state.
The model proposes actions or produces answers; it does not grant authority,
enforce security policy, or determine whether external effects actually occurred.

Scoped memories are application-managed text attached to a laboratory,
project, or agent. Structural facts are deterministic; a successful plan stores
one bounded model-generated summary. They are stored as project data and
treated as untrusted context: the application selects a bounded subset for
planning or questions, delimits them in the prompt, and never interprets them
as instructions or authorization. The model cannot choose what to store, edit,
or delete.

Projects may carry an optional GitHub source URL. The GitHub credential is a
global secret stored in `settings.yaml`, separate from the DeepSeek planning
provider. Creating a project with a GitHub source clones it into `.workspace/`
through the local `git` executable; no other GitHub operation is part of the
initial release. The source-control provider is configured separately from the
AI provider.

Prefer a deterministic workflow without a model whenever it satisfies the
requirement.

The execution workflow is a separate application component: after planning it
selects pending assigned stories, creates one git worktree and branch per
story, runs the bounded action loop, commits the result, asks the reviewer for
one bounded review, and owns the `IN_REVIEW`/`DONE` transitions and the outcome
row. The model proposes actions; the application validates paths, executes
them, and never lets the model choose a state transition (ADR-0050).

## Components

```text
User / Calling Application
           |
           v
    Application Boundary
           |
           v
       Orchestrator --------> Model Adapter
           |
           v
   Validated Action Proposal
           |
           v
  Policy and Approval Checks
           |
           v
      Tool Dispatcher ------> External Resources
           |
           v
     Validated Tool Result
           |
           +---------------> Orchestrator
