# Codex Ticket Probe

This separate executable adapts the ticket request and connection-trace behavior of
Wei-Shaw/sub2api, including `backend/internal/service/openai_codex_ticket.go` and
`backend/internal/repository/openai_codex_harvest_transport.go` from the locally
customized deployment based on commit 49a39b6dc1abed30fd227611e8af1108bc427610.

This component is distributed under LGPL-3.0-only. See COPYING and COPYING.LESSER.
The independent Rust gateway retains its own license. The helper is replaceable;
communication uses JSON on anonymous stdin/stdout pipes, not a linked library.

Changes on 2026-09-19: standalone JSON interface, bounded input/output, no credential
logging, Go 1.27.0 toolchain, and tests for request headers, connection identity,
fresh attempts, trace failures and ticket validation. Account selection, OAuth,
caching and scheduling remain in the Rust gateway.

Build with `CGO_ENABLED=0 GOTOOLCHAIN=go1.27.0 go build -trimpath -o codex-ticket-probe .`.
Run `go test ./...` in this directory. Place the executable alongside codex-proxy-rs,
or set CPR_CODEX_TICKET_PROBE to its absolute path. Do not run real credentials
through shell arguments or save them in request fixtures.
