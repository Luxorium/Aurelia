# Region Threading

Aurelia is inspired by Folia's regionized model, but it does not copy Folia or
Paper code.

## Ownership Model

A region owns:

- Chunks.
- Entities.
- Tile entities.
- Scheduled block ticks.
- Local region tasks.

A region may only mutate its own data from its owning tick thread. During
development, wrong-thread access should throw loudly so unsafe code is found
early.

## Cross-Region Work

Cross-region actions must be scheduled into the target region mailbox. Code that
needs to affect another region should enqueue a task and let the target region
run that task on its owning tick thread.

## First Implementation

The first implementation uses fixed region sections. Advanced merge/split logic
is intentionally deferred until the basic ownership model, mailboxes, and
thread checks are proven.

## TODOs

- Define fixed region section dimensions.
- Add safe entity transfer between regions.
- Add scheduled block tick ownership.
- Add diagnostics for blocked or overloaded region mailboxes.
- Design region merge/split behavior after fixed regions are stable.
