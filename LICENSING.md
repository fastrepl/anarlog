# Licensing and product boundary

Anarlog uses a repository boundary, not a source-available license, to separate the open-source application from Fastrepl's commercial enterprise software.

## Open-source repository

Everything in this repository is licensed under the [MIT License](LICENSE) unless a file or bundled third-party component says otherwise. This includes:

- the desktop, web, mobile, and command-line applications;
- local meeting capture, transcription, storage, export, and sync clients;
- provider-neutral meeting-capture contracts, normalized events, and behavior fixtures; and
- public APIs and SDKs required for an MIT client to communicate with a separately deployed service.

Contributions accepted into this repository must be distributable under MIT. A contribution must not contain Fastrepl enterprise source code, credentials, customer configuration, or third-party code whose terms are incompatible with MIT distribution.

## Commercial enterprise software

Fastrepl enterprise software is developed and distributed from a separate private repository under a commercial agreement. It includes:

- customer-hosted meeting-bot workers and their browser automation;
- scheduling, orchestration, capacity management, and job persistence;
- enterprise administration, SSO enforcement, audit export, policy, and license enforcement; and
- Docker, Kubernetes, air-gapped, and regulated-environment deployment packages.

Enterprise software may depend on versioned MIT packages from this repository. Code in this repository must never depend on, import, generate from, or require the enterprise repository.

Enterprise source delivery, binary delivery, evaluation rights, redistribution, support, warranty, and termination terms must come from a counsel-approved Fastrepl Enterprise Software License and the applicable customer agreement. Those terms are intentionally not defined in this MIT repository.

## Self-hosted licensing

Customer-hosted enterprise deployments must support a signed, time-bounded or perpetual license file that can be validated offline. The deployment must not require an outbound licensing request after installation unless the customer explicitly enables one. Activation, enforcement, and signing-key material belong only in the enterprise repository.

## Third-party provenance

Before third-party code or assets are incorporated into either product:

1. Record the upstream repository, immutable revision, file paths, copyright holder, license, and whether the material was copied, modified, or used only as behavioral reference.
2. Preserve all required license, copyright, patent, trademark, modification, and NOTICE material in the delivered source or binary distribution.
3. Keep provenance records beside the consuming code and generate the distribution's third-party notices from those records.
4. Do not incorporate AGPL, GPL, SSPL, source-available, or unknown-license material without written legal approval.

Apache-2.0 material can be used in commercial software when its conditions are satisfied. That does not make untracked copying acceptable: directly reused files must retain the required notices and modifications must be identified.

Behavioral observation is preferred when replacing a third-party implementation. Tests and fixtures should record public API behavior without copying implementation source, confidential data, or trademarks into product identity.

## Component placement test

A component belongs in this MIT repository only when all of the following are true:

- an independent MIT client needs the component;
- the component is useful without Fastrepl's enterprise control plane;
- publishing it does not expose enterprise enforcement, orchestration, or deployment logic; and
- every dependency and incorporated asset is compatible with MIT distribution.

If any condition is false, build it in the enterprise repository or request a licensing review before implementation.
