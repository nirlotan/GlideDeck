# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

GlideDeck's Windows installer (`GlideDeck-<version>-x64.msi`) and the program inside it
(`glidedeck.exe`) are signed with a certificate issued to SignPath Foundation. The macOS
app is signed with an Apple Developer ID and notarized by Apple.

## What gets signed

Only files built from this repository's source code, by the
[Release workflow](.github/workflows/release.yml) on GitHub Actions, are signed. Nothing
built on a personal computer is ever signed. Each release is signed only after a person
listed below as an approver has reviewed and approved the signing request.

## Team roles

- Committers and reviewers: [nirlotan](https://github.com/nirlotan)
- Approvers: [nirlotan](https://github.com/nirlotan)

All team members use multi-factor authentication for GitHub and SignPath. Changes from
anyone else arrive as pull requests and are reviewed before they're merged.

## Privacy policy

This program will not transfer any information to other networked systems unless
specifically requested by the user or the person installing or operating it. See
[PRIVACY.md](PRIVACY.md) for exactly when GlideDeck goes online.
