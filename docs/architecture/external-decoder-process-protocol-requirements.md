# External Decoder Process Protocol Requirements

Status: Requirements for a future milestone. Stab 0.2 does not implement this transport and does not promise a wire-compatible schema.

## Purpose

A future external-process decoder boundary should let Python, another language runtime, or an independently released decoder participate in the same truth-hidden detection-to-prediction workflow as the Rust `DecoderSession` contract without freezing a Rust ABI.

## Required Semantics

- The protocol must negotiate an explicit protocol version, Stim compatibility target, supported detector and correction layouts, maximum batch dimensions, and optional deterministic capabilities before model compilation.
- Model compilation must bind canonical DEM bytes, the model fingerprint, detector count, correction width, and implementation-owned resource limits into an opaque process-local session identifier. Serialized executable plans are outside the contract.
- Decode requests must contain a unique request identifier, session identifier, exact shot count, detector-only packed records, and an explicit byte order and tail-bit policy. Observable truth must never cross the decoder input boundary.
- Decode responses must identify the request, return only the complete admitted observable-prediction payload, and report completed shots or one typed failure. The initial protocol has no in-band cancellation frame, acknowledgement, or trusted partial-response contract.
- Reused sessions must keep a fixed layout. A process restart invalidates all prior process-local session identifiers.

## Cancellation Decision

- Initial transport cancellation is controller-owned process-group termination. The controller transitions the request to cancelled, terminates the decoder process group, invalidates every session owned by that process, and does not wait for or infer an in-band acknowledgement.
- A fully framed and semantically validated response wins only when the controller accepts it before the cancellation-state transition. Any response bytes observed after cancellation starts are discarded, and caller-owned prediction storage is not partially committed from them.
- This kill-only rule deliberately differs from the in-process Rust session's documented prefix cancellation because a truncated or concurrent external byte stream cannot safely prove a committed prefix. A future graceful-cancellation capability must be an explicitly negotiated versioned extension with its own acknowledgement, prefix, race, and recovery semantics.

## Framing And Resource Safety

- Framing must be length-delimited and versioned. Every frame type must have a configured maximum before payload allocation, and unknown required fields or message kinds must fail closed.
- Standard output must carry protocol frames only. Human diagnostics may use standard error, but machine failures must use bounded stable codes and structured context inside the protocol.
- The controller must bound input, output, diagnostic capture, resident memory where supported, file creation, wall time, and descendant process lifetime. Timeout, controller cancellation, malformed frames, truncated frames, extra frames, and retained descendant pipe handles must terminate the complete process group.
- The child must not depend on the caller's working directory, inherited package configuration, network access, secrets, or ambient environment. A future implementation must define the exact minimal inherited environment.
- Input and output buffers must scale with the negotiated model and active batch, never the total experiment length. Limits must distinguish model compilation, retained session state, per-batch work, and protocol bytes.

## Identity And Reproducibility

- Handshake evidence must identify the decoder implementation, implementation version, protocol version, build identity, and advertised deterministic behavior.
- Model and request identities must be architecture-independent. Opaque session identifiers may be process-local and nondeterministic because they are not evidence identities.
- A deterministic implementation must reproduce predictions for identical compiled model, detector input, and declared options. A stochastic implementation must explicitly negotiate and bind its random policy.
- Conformance must include an independent Rust-session comparison over exact results, typed failures, zero-shot behavior, malformed frames, resource limits, process crashes, and process restarts. Kill-only cancellation conformance must separately prove process-group termination, complete session invalidation, response-versus-cancellation ordering, discarded late bytes, and unchanged caller output when cancellation wins.

## Explicit Non-Goals

- No dynamic Rust library ABI.
- No runtime gate registration or extension instruction transport.
- No general simulator RPC protocol.
- No serialized private decoder or simulator executable plan.
- No protocol implementation, schema number, or compatibility promise in Stab 0.2.
