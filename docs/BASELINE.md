# COGNIX_OS — immutable requirements baseline

The conversation's Master Architecture and Requirements Specification v1.0 (62 sections) and 15 development rules remain authoritative. This commit is an initial implementation, not a release or replacement specification. No requirement is removed. Changes require owner approval. All development on develop; main untouched.

## Integrated target
Scratch-built UEFI bootloader and native memory-safe capability microkernel; physical and virtual memory, interrupts, IPC, syscalls, process/thread lifecycle, multicore, scheduling, hardware abstraction and isolated drivers; conventional RAM, dedicated VRAM and hardware unified memory (no false physical unification), NUMA and acceleration; storage, versioned filesystem, snapshots, encryption, networking, power, thermal, GPU/NPU; native graphical desktop, accessibility, multilingual voice/text/GUI, normal manual operation and offline use; installer, recovery, package manager, signed updates and marketplace; compatibility via ports, runtimes, emulation or VMs subject to licenses and hardware.

AI: local Ollama-compatible runtime, cloud adapters, hybrid and multi-agent routing, persistent context, model manager, prediction and feedback, intent compiler, planning, independently authorized execution, verification, transactional recovery, live observable action trace, user-controlled cloud data and budgets. AI never bypasses kernel permissions. Installed app control is default-deny, app/action scoped, revocable, via API/accessibility/GUI adapters, with explicit approval for sensitive operations. Cloud private reasoning is not promised.

Developer ecosystem: IDE, terminal, compilers, debuggers, profilers, Git, SDKs, build/test/deployment, environments, containers/VMs, databases, web/mobile, AI/ML, research/data science, game/graphics, embedded/IoT/robotics, VLSI/FPGA, OS/kernel/driver, cloud/DevOps, authorized cybersecurity labs, self-hosting. Tools modular and on demand. Productivity, file management, cross-device, app compatibility and public ISO remain targets.

## Evidence and gaps
- core: no_std-compatible policy and resource-accounting library; simulated process records, scoped grants, request queue and audit events. Not real process execution, kernel isolation or physical memory management.
- simulator: host integration scenario.
- boot: standalone UEFI console entry, not a kernel loader; no secure boot, recovery or emulator evidence yet.
- Planner trait: interface only; no actual model provider implemented.
- All other subsystems and real integration, hardware and security verification remain open. No novelty or production security claim.
- Grants currently rely on a trusted in-process caller; future kernel boundary must enforce authenticated actors, unforgeable capability handles, revocation at execution, persistent protected audit and actual adapter isolation.
- Tests are authored, not presumed passing. No user credentials, purchases or physical disk changes required for this commit.
