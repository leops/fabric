Fabric
===============

An experiment on extending the Source engine with a WebAssembly runtime, see
[ChaosInitiative/Portal-2-Community-Edition#421](https://github.com/ChaosInitiative/Portal-2-Community-Edition/issues/421)
for an in depth context / explanation

# Features

For this version the Fabric runtime is compiled into a server plugin DLL loaded
by the engine. The features exposed to the WASM modules are a (very small) subset
of the APIs exposed to server plugins, namely registering event listeners and
inspecting received event objects. The aim here is not to provide a workable
environment for scripting (yet), but to showcase the feasibility of integrating
WASM code with Source with both calls from WASM to then engine and from the
engine to WASM.

# Backend

Right now this project uses Cranelift as a "production" backend for emitting machine code.
This allows a greater control over the emitted code compared to over "off-the-shelf" WASM
libraries, and in turns permits relaxing the threading constraints of the WASM environment
(specifically that all WASM code can only be called from a single thread, a constraint difficult
to enforce when integrating an existing software like Source without hurting performances).
But this also means a bunch of WASM feature are left unimplemented (for now), and debugging
the emitted code is nearly impossible.

In the near future I'll add an alternative "debugging" backend using V8. Since V8 needs
to be run from a single thread this version will certainly have an important performance
overhead, but will allow debugging the WASM code using the existing Chrome Devtools.
