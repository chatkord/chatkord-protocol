# Security

## Reporting a vulnerability

Report it privately through this repository's **Security** tab ("Report a vulnerability"), never in a public issue,
pull request or discussion. Say what an attacker can reach, how, and what they gain, with the smallest steps that
show it.

You will get an answer within a week. A fix is made in private, released, and then the advisory is published with
your name unless you ask otherwise.

## Scope

This repository holds what both ends of a Chatkord session parse and write: the framing, the bounds every length is
checked against, the redaction of secret fields and of log lines, and the rules both ends apply. A report that gets a
peer to make either end panic, allocate without bound, accept what the schema refuses, or print a secret is in scope.
How each app defends the rest is its own repository's.

## Supported versions

Only the latest release is supported: a fix is not back-ported.
