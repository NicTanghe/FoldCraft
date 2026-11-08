# Software factory

This fork uses a local implementation queue and feature pull requests. The old
upstream coordinator and its approvals are not part of this workflow.

## Agent communications

`comms/` is a gitignored local mailbox. Create it with `mkdir -p comms` in a new
checkout (Git does not preserve ignored directories). Use one Markdown file per
agent/session, for example `comms/geometry-agent.md`. Include the ticket ID,
branch/worktree, current work, findings, blockers and a handoff. Read relevant
messages before overlapping work. Keep secrets out of messages. Move lasting
technical decisions into tracked documentation or the PR.

All agents on this machine use one dispatcher checkout for the queue and mailbox.
Workers in other worktrees use `scripts/factory.py --root /path/to/dispatcher ...`.
Do not maintain competing queue copies or merge divergent scheduling histories.
The local lock serializes queue writes in that checkout; it does not coordinate
separate machines. A stale `comms/queue.lock` should be removed only after checking
that its recorded process is no longer writing. Claims never expire silently.

## Implementation queue

`implementation/queue.json` is the authoritative, tracked queue. Roadmaps and
`docs/rewrite-backlog.md` are sources of ideas, not alternate queues. Add a ticket
with observable acceptance criteria before implementing a feature. Keep effort
estimates small enough for useful checkpoints. One point is a consistent unit of
estimated implementation effort, not elapsed time, tokens or lines of code.

```sh
python3 scripts/factory.py add "Feature title" --points 3 --priority high --acceptance "Observable result and failure cases"
python3 scripts/factory.py next
python3 scripts/factory.py start --owner agent-session
python3 scripts/factory.py charge FC-0001 --owner agent-session --points 1
python3 scripts/factory.py release FC-0001 --owner agent-session
python3 scripts/factory.py priority FC-0001 urgent
python3 scripts/factory.py check
```

`start` atomically selects and owns an eligible ticket; it requires no external
approval. `next` is read-only and includes active tickets so agents can see why
work might need to reach a checkpoint before more work becomes eligible. Charge
actual effort at meaningful checkpoints, including unsuccessful work, then release
for reselection when a slice finishes. Do not interrupt unsafe partial transactions
just to match a point boundary. Estimates may be exceeded; spent points never
imply completion. Use `--depends-on FC-0001` when adding a dependent ticket.
Dependencies must be merged before dependent tickets become runnable. Unrelated
agents may work concurrently, but check mailbox messages and isolate code changes
in separate worktrees. Commit queue updates from the dispatcher; workers must not
copy an old queue snapshot into their feature branches.

### EEVDF-inspired scheduling

This is a cooperative ticket scheduler, not Linux's kernel scheduler. It uses
weighted virtual service, eligibility and virtual deadlines:

- Priority weights: low = 1, normal = 2, high = 4, urgent = 8.
- A ticket's virtual runtime increases by `charged_points / weight`.
- Virtual time `V` is the weighted mean runtime of runnable ready/active tickets.
- A ticket is eligible when its runtime is at most `V` (nonnegative service lag).
- Among eligible ready tickets, select the earliest deadline `runtime + 1 / weight`.
  Ticket ID breaks ties deterministically. The scheduling slice is one point.
- New tickets enter at current virtual time. Priority changes preserve weighted
  lag, clamped to nonnegative runtime. Review fixes reenter at no earlier than
  current virtual time; completing a review does not earn idle-service credit.

High priority gets an earlier initial deadline and a larger long-term share;
urgent gets four times the share of normal. Low-priority work continues to get
service in a finite runnable set when workers charge and release regularly.
This is not a starvation guarantee under unlimited arrivals, stalled owners or
unreported effort. Dependencies, review waits and non-preemptible implementation
steps differ from CPU scheduling. A boost changes weight, not dependency readiness.

## Feature delivery and reviews

Every feature is implemented on a branch and delivered through a PR in this fork.
The owner authorizes the ordinary commit, push, PR creation, Codex ticket/PR review,
review-fix and `gh` merge steps for assigned feature work. Do not ask for a fresh
permission at every step. Host permissions and applicable branch rules still apply.
This does not authorize unrelated releases, destructive history changes or GPU runs.

1. Start the selected ticket, create a feature branch/worktree, implement it and
   run relevant checks. Keep the physical and document invariants in `AGENTS.md`.
2. Commit and push the feature branch. Create the PR using `gh pr create`, with
   the ticket ID, acceptance results and limitations. For multiline text, use
   `--body-file`. Target this fork's actual default branch.
3. Attach the PR with `python3 scripts/factory.py review FC-0001 --owner agent-session --pr URL`.
4. Codex ticket and PR reviews are always allowed, including during implementation.
   Inspect findings and fix those that make sense: reproducible defects, violated
   contracts and useful scoped improvements. Explain why a finding is declined;
   do not blindly apply suggestions or silently dismiss them. For more work after
   entering review, use `reopen`, then schedule/charge it normally. Revalidate changes.
   If a configured review integration is unavailable, record that limitation and
   perform a local review; do not claim an external review happened.
5. Inspect the current PR head and checks with `gh pr view` and `gh pr checks`.
   Resolve relevant findings and conflicts, then merge using `gh pr merge URL --squash`.
   Do not bypass failed checks or branch protections with `--admin`. If rules
   require approval, wait for that approval rather than treating standing task
   authorization as a GitHub reviewer approval.
6. Run `python3 scripts/factory.py done FC-0001`. This queries GitHub using `gh`
   and refuses completion until the recorded PR is merged. Persist the updated
   queue and handoff in the dispatcher. Queue bookkeeping can be committed directly;
   it is not a feature and does not require a recursive bookkeeping PR.

Agents may review each other's work through the mailbox. This document permits
reviews but does not launch agents or configure a hosted Codex integration.
GitHub authentication must already be available for remote actions; when it is
missing, finish the local work and report the blocked publication/merge step.

## Validate the factory

```sh
python3 -m unittest discover -s scripts -p 'test_factory.py'
python3 scripts/factory.py check
```

These are small CPU-only checks. Rust, desktop and packaging workflows remain
independent of queue ownership. No GPU test is added to the default suite.
