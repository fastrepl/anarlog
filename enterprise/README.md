# Anarlog Enterprise

This directory contains source-visible, commercially licensed Anarlog Enterprise components. It is part of the Anarlog monorepo but is not licensed under the repository's MIT License. See [LICENSE](LICENSE) and the repository's [licensing boundary](../LICENSING.md).

Enterprise packages may depend on the MIT community layer. Community packages must never depend on this directory. Shared contracts and provider-neutral interfaces needed by independent clients belong in the community layer.

Customer configuration, credentials, license-signing keys, and confidential deployment material must never be committed. Third-party material requires complete provenance and must retain its original notices.
