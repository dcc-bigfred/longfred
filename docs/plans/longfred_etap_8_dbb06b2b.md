---
name: longfred etap 8
overview: "Stage 8: domain model + control state machine. Task `domain::task` consumes `INPUT_CHANNEL` and `WIT_EVENTS`, maps InputEvent -> Action -> WiThrottle commands (to `WIT_COMMANDS`), reduces ServerEvent to state, publishes `DomainSnapshot` to UI. Numeric acquisition (digits + #), consist-aware direction (full MU facing). DoD: cargo build + test -p longfred-proto; on hardware address acquisition, encoder speed change, direction, functions, e-stop."
todos: []
isProject: false
---

# Stage 8 — Domain Model + Control State Machine

## Goal and DoD
A domain layer is created: state model (throttle, consist, speed, direction, functions, roster, power) + task `domain::task`, which:
