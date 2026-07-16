---
name: longfred etap 7
overview: "Stage 7: WiThrottle TCP client + protocol loop. Task connects TCP to WIT_SERVER (from Stage 5), handshake (N{name}, HU{id}, requireHeartbeat), reads lines, parses via longfred-proto and emits ServerEvent to WIT_EVENTS channel (for domain, Stage 8). Outgoing commands from WIT_COMMANDS channel. Periodic heartbeat (* every period from server) + no-response watchdog + reconnect. DoD: connection to real server (JMRI/DCC-EX), handshake, heartbeat, version/roster reception visible in log."
todos: []
isProject: false
---

## Stage 7 — TCP Client + WiThrottle Protocol Loop

### Goal and DoD
After selecting the server (`net::WIT_SERVER`, Stage 5) the client task establishes a TCP connection, performs handshake (`N{
