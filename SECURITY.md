# Security Policy

## Reporting Security Vulnerabilities

We take the security of Waymesh nodes, participants, and passenger privacy seriously. If you discover a security vulnerability, please do NOT file a public issue.

Instead, please send an encrypted or direct disclosure report to:
`security@waymesh.org` (or contact repository maintainers directly).

Please include:
* Vulnerability description and impact assessment
* Reproduction steps or proof of concept
* Affected crates or endpoints

## Scope & Security Architecture

1. **Strict Gender Separation Guarantee:**
   * In passenger ride matching, male and female participants must never be paired.
   * Any bypass of this rule is considered a critical security vulnerability.

2. **Two-Stage Cryptographic OTP Custody:**
   * Physical custody transfers require 6-digit cryptographic OTP verification.
   * Rate limiting: A maximum of 3 failed verification attempts is allowed before the booking or parcel enters a locked dispute state.

3. **No Cryptographic Fund Custody:**
   * Waymesh nodes do not hold, escrow, or transmit cryptocurrency or volatile tokens.
   * Peer transactions occur off-node directly between parties privately via physical cash, direct peer fiat (UPI), or mutual private agreement.
