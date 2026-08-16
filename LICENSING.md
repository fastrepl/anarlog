# Licensing and product boundary

Anarlog is developed in a mixed-license monorepo. The community application and shared contracts remain open source under MIT, while source-visible enterprise components are commercially licensed.

This is path-based mixed licensing, not a choice between two licenses for the same file.

## License scope

| Path | License |
| --- | --- |
| `enterprise/**` | [Anarlog Enterprise Commercial License Notice](enterprise/LICENSE) and the applicable written agreement with Fastrepl, Inc. |
| Everything else | [MIT License](LICENSE), unless a nearer license or third-party notice says otherwise |

The nearest license file controls. Moving code across the `enterprise/` boundary requires an explicit licensing review; a file does not keep its previous license merely because it was moved.

Public availability of enterprise source does not place it under MIT or make it open source. Access, evaluation, production use, modification, redistribution, support, warranty, and termination rights come only from a written agreement with Fastrepl, Inc. The commercial notice is a boundary marker, not a substitute for customer terms reviewed by counsel.

## MIT community layer

The MIT layer includes:

- the desktop, web, mobile, command-line, and community server applications;
- local meeting capture, transcription, storage, export, and sync clients;
- provider-neutral contracts, normalized events, behavior fixtures, and ingestion interfaces; and
- public APIs and SDKs required for independently developed clients and integrations.

The MIT layer must build and test without `enterprise/`. It must never import, depend on, generate from, or require a commercially licensed package. Shared contracts needed on both sides of the boundary belong in the MIT layer.

## Commercial enterprise layer

Commercial components live only in `enterprise/`. They may depend on MIT packages from this repository and may provide customer-hosted orchestration, meeting-capture services, administration, deployment, policy, or license enforcement.

Keeping both layers in one monorepo permits atomic contract changes and end-to-end tests. It does not change the dependency direction: enterprise may depend on MIT; MIT may not depend on enterprise. CI enforces that rule for Rust and JavaScript package manifests.

## Contributions

Contributions outside `enterprise/` must be distributable under MIT. Do not submit changes under `enterprise/` unless Fastrepl has confirmed the applicable contribution terms in writing. Public pull requests do not, by themselves, grant Fastrepl the additional rights needed to distribute a contribution commercially.

Never submit credentials, customer configuration, confidential material, or third-party code without compatible rights and complete provenance.

## Self-hosted licensing

Customer-hosted enterprise deployments should support signed, time-bounded or perpetual licenses that can be validated offline. They must not require an outbound licensing request after installation unless the customer explicitly enables one. Activation, enforcement, and signing-key material belong in `enterprise/`; public verification contracts may remain in the MIT layer when independently useful.

## Third-party provenance

Before third-party code or assets are incorporated into either layer:

1. Record the upstream repository, immutable revision, file paths, copyright holder, license, and whether the material was copied, modified, or used only as behavioral reference.
2. Preserve all required license, copyright, patent, trademark, modification, and NOTICE material in the delivered source or binary distribution.
3. Keep provenance records beside the consuming code and generate the distribution's third-party notices from those records.
4. Do not incorporate AGPL, GPL, SSPL, source-available, or unknown-license material without written legal approval.

Third-party components remain under their original licenses regardless of which Anarlog layer consumes them.
