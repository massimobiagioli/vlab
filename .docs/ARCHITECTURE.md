# Architecture

## Status

**Proposed baseline — not a description of verified implementation.**

This document assumes a Rust application that uses an AI agent to complete
bounded tasks through explicitly authorized tools.

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

- Multi-agent orchestration.
- Distributed execution.
- Autonomous permission escalation.
- Arbitrary shell execution.
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
