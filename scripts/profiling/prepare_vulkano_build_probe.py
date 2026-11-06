#!/usr/bin/env python3
"""Prepare an isolated Vulkano 0.35.2 copy with opt-in build phase timers.

Does not edit the registry, project manifest, or lockfile. Use a temporary Cargo
patch to build against the printed path. This is diagnostic instrumentation,
not a maintained Vulkano fork. See the rendering optimization epic.
"""
import argparse
from pathlib import Path
import shutil


PROBE = r'''
// Temporary CPU diagnostic probe; no rendering behavior changes.
struct BuildPhaseProbe {
    enabled: bool,
    clock: std::time::Instant,
    ns: [u64; 8],
}

impl BuildPhaseProbe {
    fn new() -> Self {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let enabled = *ENABLED.get_or_init(||
            std::env::var_os("MITTENS_VULKANO_BUILD_PROBE").is_some());
        Self { enabled, clock: std::time::Instant::now(), ns: [0; 8] }
    }

    fn phase(&mut self, index: usize) {
        if self.enabled {
            let now = std::time::Instant::now();
            self.ns[index] = now.duration_since(self.clock).as_nanos() as u64;
            self.clock = now;
        }
    }

    fn emit(&self, level: CommandBufferLevel, commands: usize, resources: usize) {
        if self.enabled {
            eprintln!(concat!(
                "VULKANO_BUILD_PROBE {{\"level\":\"{:?}\",\"commands\":{},\"resource_uses\":{},",
                "\"initialization_ns\":{},\"sync_analysis_ns\":{},\"sync_finalize_ns\":{},",
                "\"emit_commands_ns\":{},\"emit_final_barriers_ns\":{},\"driver_end_ns\":{},",
                "\"keepalive_collect_ns\":{},\"builder_state_drop_ns\":{}}}"),
                level, commands, resources, self.ns[0], self.ns[1], self.ns[2], self.ns[3],
                self.ns[4], self.ns[5], self.ns[6], self.ns[7]);
        }
    }
}
'''


def prepare(source: Path, target: Path) -> Path:
    if target.exists():
        raise SystemExit(f"Target already exists: {target}; choose a fresh directory")
    manifest = (source / "Cargo.toml").read_text()
    if 'version = "0.35.2"' not in manifest:
        raise SystemExit("Probe supports Vulkano 0.35.2 only")
    shutil.copytree(source, target)
    path = target / "src/command_buffer/auto/builder.rs"
    original = path.read_text()
    start = original.index("    unsafe fn end_unchecked(\n")
    end = original.index("    /// Returns the level of the command buffer.", start)
    method = original[start:end]
    method = method.replace(
        "        let mut auto_sync_state = AutoSyncState::new(",
        """        let level = self.inner.level();
        let commands = self.commands.len();
        let resources = self.commands.iter().map(|(c, _)| c.used_resources.len()).sum();
        let mut probe = BuildPhaseProbe::new();
        let mut auto_sync_state = AutoSyncState::new(""", 1)
    for anchor, index in [
        ("        // Add barriers between the commands.", 0),
        ("        let (mut barriers, resources_usage, secondary_resources_usage)", 1),
        ("        // Record all the commands and barriers", 2),
        ("        // Record final barriers", 3),
        ("        debug_assert!(barriers.is_empty());", 4),
    ]:
        assert anchor in method, anchor
        method = method.replace(anchor, f"        probe.phase({index});\n\n" + anchor, 1)
    old_result = '''        Ok((
            unsafe { self.inner.end() }?,
            self.commands
                .into_iter()
                .map(|(_, record_func)| record_func)
                .collect(),
            resources_usage,
            secondary_resources_usage,
        ))'''
    assert old_result in method
    method = method.replace(old_result, '''        let AutoCommandBufferBuilder {
            inner, commands: recorded_commands, builder_state, marker: _,
        } = self;
        let inner = unsafe { inner.end() }?;
        probe.phase(5);
        let keepalive = recorded_commands.into_iter().map(|(_, record_func)| record_func).collect();
        probe.phase(6);
        drop(builder_state);
        probe.phase(7);
        probe.emit(level, commands, resources);
        Ok((inner, keepalive, resources_usage, secondary_resources_usage))''', 1)
    path.write_text(original[:start] + original[end:] + '\ninclude!("build_probe_end.rs");\n')
    (path.parent / "build_probe_end.rs").write_text(
        PROBE + "\nimpl<L> AutoCommandBufferBuilder<L> {\n" + method + "}\n")
    return target


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="Registry Vulkano 0.35.2 directory")
    parser.add_argument("target", type=Path, help="Fresh writable temporary directory")
    args = parser.parse_args()
    print(prepare(args.source, args.target))
