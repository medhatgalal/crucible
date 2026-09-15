## Goal
product/hello.txt contains exactly hello
## Non-goals
live systems, extra modules, network
## Owned files
- product/hello.txt
## Test files
- (none)
## Acceptance criteria
- product/hello.txt contains exactly hello
- maker-authored falsifier is grep -qx hello product/hello.txt
## Focused falsifier
MAKER-WRITES
## Stop conditions
stop-ask on live write
## Risk
LOW
SPEC-AUTHOR: operator
