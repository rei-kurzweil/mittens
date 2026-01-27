use crate::engine::ecs;
use slotmap::KeyData;
use super::{pipe, util};
use std::io::Write;

/// Runs REPL commands against engine state.
///
/// This is intended to be called from the main thread (e.g. inside `Universe::update()`),
/// after commands are received from the stdin thread.
pub struct ReplBackend {
    cwd: Option<ecs::ComponentId>,
}

impl ReplBackend {
    pub fn new() -> Self {
        Self { cwd: None }
    }

    pub(crate) fn cwd(&self) -> Option<ecs::ComponentId> {
        self.cwd
    }

    fn format_component_id_short(id: ecs::ComponentId) -> String {
        let s = format!("{:?}", id);
        if let (Some(l), Some(r)) = (s.find('('), s.rfind(')')) {
            if r > l + 1 {
                return s[l + 1..r].to_string();
            }
        }
        s
    }

    fn parse_component_id_short(s: &str) -> Option<ecs::ComponentId> {
        // slotmap::KeyData debug format is "<idx>v<version>".
        let (idx_str, ver_str) = s.split_once('v')?;
        let idx: u32 = idx_str.parse().ok()?;
        let version: u32 = ver_str.parse().ok()?;
        let ffi = (u64::from(version) << 32) | u64::from(idx);
        Some(KeyData::from_ffi(ffi).into())
    }

    pub(crate) fn current_listing(&self, world: &ecs::World) -> Vec<ecs::ComponentId> {
        self.listing_for(world, self.cwd)
    }

    fn listing_for(
        &self,
        world: &ecs::World,
        cwd: Option<ecs::ComponentId>,
    ) -> Vec<ecs::ComponentId> {
        match cwd {
            None => world
                .all_components()
                .filter(|&cid| world.parent_of(cid).is_none())
                .collect(),
            Some(cwd) => world.children_of(cwd).to_vec(),
        }
    }

    fn resolve_in_listing(
        &self,
        world: &ecs::World,
        listing: &[ecs::ComponentId],
        segment: &str,
    ) -> Result<ecs::ComponentId, String> {
        let (key_part, name_part) = segment.split_once(':').unwrap_or((segment, ""));

        // 1) Numeric index into listing.
        if let Ok(idx) = key_part.parse::<usize>() {
            let cid = listing
                .get(idx)
                .copied()
                .ok_or_else(|| format!("index out of range: {}", idx))?;
            if !name_part.is_empty() {
                let actual_name = world
                    .get_component_node(cid)
                    .map(|n| n.name.as_str())
                    .unwrap_or("<deleted>");
                if actual_name != name_part {
                    return Err(format!(
                        "index {} resolved, but name mismatch: expected '{}' got '{}'",
                        idx, name_part, actual_name
                    ));
                }
            }
            return Ok(cid);
        }

        // 2) GUID.
        if let Ok(guid) = key_part.parse::<uuid::Uuid>() {
            for cid in listing.iter().copied() {
                if let Some(node) = world.get_component_node(cid) {
                    if node.guid == guid {
                        if !name_part.is_empty() && node.name != name_part {
                            return Err(format!(
                                "guid {} resolved, but name mismatch: expected '{}' got '{}'",
                                guid, name_part, node.name
                            ));
                        }
                        return Ok(cid);
                    }
                }
            }
            return Err(format!("guid not found: {}", guid));
        }

        // 3) Short ComponentId token (e.g. 7v1).
        if let Some(cid) = Self::parse_component_id_short(key_part) {
            if listing.iter().any(|&c| c == cid) {
                if !name_part.is_empty() {
                    let actual_name = world
                        .get_component_node(cid)
                        .map(|n| n.name.as_str())
                        .unwrap_or("<deleted>");
                    if actual_name != name_part {
                        return Err(format!(
                            "id {} resolved, but name mismatch: expected '{}' got '{}'",
                            key_part, name_part, actual_name
                        ));
                    }
                }
                return Ok(cid);
            }
            return Err(format!("id not in current listing: {}", key_part));
        }

        // 4) Name.
        let mut matches: Vec<ecs::ComponentId> = Vec::new();
        for cid in listing.iter().copied() {
            if let Some(node) = world.get_component_node(cid) {
                if node.name == key_part {
                    matches.push(cid);
                }
            }
        }

        match matches.len() {
            0 => Err(format!("not found: {}", key_part)),
            1 => Ok(matches[0]),
            _ => Err(format!(
                "ambiguous name: {} (use 'ls' + index, guid, or id token)",
                key_part
            )),
        }
    }

    fn cd_path(&self, world: &ecs::World, path: &str) -> Result<Option<ecs::ComponentId>, String> {
        let is_abs = path.starts_with('/');
        let mut cur: Option<ecs::ComponentId> = if is_abs { None } else { self.cwd };

        let segments = path.split('/').filter(|s| !s.is_empty());
        for seg in segments {
            match seg {
                "." => {}
                ".." => {
                    cur = cur.and_then(|cwd| world.parent_of(cwd));
                }
                _ => {
                    let listing = self.listing_for(world, cur);
                    let next = self.resolve_in_listing(world, &listing, seg)?;
                    cur = Some(next);
                }
            }
        }

        Ok(cur)
    }

    pub(crate) fn resolve_path_or_item(
        &self,
        world: &ecs::World,
        arg: &str,
    ) -> Result<Option<ecs::ComponentId>, String> {
        match arg {
            "/" => Ok(None),
            "." => Ok(self.cwd),
            ".." => Ok(self.cwd.and_then(|cwd| world.parent_of(cwd))),
            _ if arg.contains('/') => self.cd_path(world, arg),
            _ if arg.parse::<uuid::Uuid>().is_ok() => {
                // For single-item GUID lookups (not path segments), allow global resolution.
                // This keeps `cd /a/b/c` semantics local, but makes `cat <guid>` usable anywhere.
                let guid = arg
                    .parse::<uuid::Uuid>()
                    .map_err(|e| format!("invalid guid: {}", e))?;

                world
                    .component_id_by_guid(guid)
                    .map(Some)
                    .ok_or_else(|| format!("guid not found: {}", guid))
            }
            _ => {
                let listing = self.current_listing(world);
                Ok(Some(self.resolve_in_listing(world, &listing, arg)?))
            }
        }
    }

    /// Execute a single REPL command.
    ///
    /// This currently only reads from `world` and updates internal REPL state (cwd).
    pub fn exec(&mut self, world: &ecs::World, cmd: &str) {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            return;
        }

        // Pipe system (component-object pipes).
        if cmd.contains('|') {
            match pipe::try_exec_piped(self, world, cmd) {
                Ok(true) => return,
                Ok(false) => {}
                Err(e) => {
                    println!("🐈 pipe: {}", e);
                    return;
                }
            }
        }

        // If the cwd component was deleted, reset to root.
        if let Some(cwd) = self.cwd {
            if world.get_component_node(cwd).is_none() {
                self.cwd = None;
            }
        }

        let mut it = cmd.split_whitespace();
        let Some(verb) = it.next() else {
            return;
        };

        match verb {
            "help" => {
                println!("🐈 Commands:");
                println!("🐈   ls");
                println!("🐈   cd <name>");
                println!("🐈   cd <index>");
                println!("🐈   cd <guid>");
                println!("🐈   cd <path>");
                println!("🐈   cd ..");
                println!("🐈   cd /");
                println!("🐈   pwd");
                println!("🐈   cat <path>");
                println!("🐈   ls | grep <pattern>");
                println!("🐈   cat <path> | grep <pattern>");
                println!("🐈   <cmd> |    (trailing pipe prints summary)");
                println!("🐈   clear");
            }
            "clear" | "cls" => {
                // Clear screen + move cursor to home. (Many terminals also treat 3J as clear scrollback.)
                print!("\x1b[2J\x1b[H\x1b[3J");
                let _ = std::io::stdout().flush();
            }
            "pwd" => {
                match self.cwd {
                    None => println!("🐈 /"),
                    Some(mut cur) => {
                        let mut parts: Vec<String> = Vec::new();
                        loop {
                            let Some(node) = world.get_component_node(cur) else {
                                break;
                            };
                            parts.push(format!(
                                "{}:{}",
                                Self::format_component_id_short(cur),
                                node.name
                            ));
                            match world.parent_of(cur) {
                                Some(p) => cur = p,
                                None => break,
                            }
                        }
                        parts.reverse();
                        println!("🐈 /{}", parts.join("/"));
                    }
                }
            }
            "ls" => {
                let ids: Vec<ecs::ComponentId> = self.current_listing(world);

                if ids.is_empty() {
                    println!("🐈 (empty)");
                    return;
                }

                for (i, cid) in ids.into_iter().enumerate() {
                    if let Some(line) = util::format_ls_line(world, i, cid) {
                        println!("{}", line);
                    }
                }
            }
            "cat" => {
                // If no arg is provided, default to the current working directory.
                // - At root (cwd=None): dump the whole scene (all roots)
                // - Otherwise: dump the cwd subtree
                let target = match it.next() {
                    None => self.cwd,
                    Some(arg) => match self.resolve_path_or_item(world, arg) {
                        Ok(t) => t,
                        Err(e) => {
                            println!("🐈 cat: {}", e);
                            return;
                        }
                    },
                };

                match target {
                    Some(root) => {
                        match ecs::ComponentCodec::encode_subtree_node(world, root)
                            .and_then(|node| {
                                serde_json::to_string_pretty(&node)
                                    .map_err(|e| format!("failed to serialize JSON: {}", e))
                            })
                        {
                            Ok(json) => println!("{}", json),
                            Err(e) => println!("🐈 cat: {}", e),
                        }
                    }
                    None => {
                        // Dump the entire scene (all roots).
                        let root_ids: Vec<ecs::ComponentId> = world
                            .all_components()
                            .filter(|&cid| world.parent_of(cid).is_none())
                            .collect();

                        let mut components = Vec::new();
                        for cid in root_ids {
                            match ecs::ComponentCodec::encode_subtree_node(world, cid) {
                                Ok(node) => components.push(node),
                                Err(e) => {
                                    println!("🐈 cat: {}", e);
                                    return;
                                }
                            }
                        }

                        let scene = ecs::component_codec::Scene { components };
                        match serde_json::to_string_pretty(&scene) {
                            Ok(json) => println!("{}", json),
                            Err(e) => println!("🐈 cat: failed to serialize JSON: {}", e),
                        }
                    }
                }
            }
            "cd" => {
                let Some(arg) = it.next() else {
                    println!(
                        "🐈 usage: cd <name> | cd <index> | cd <guid> | cd <path> | cd .. | cd /"
                    );
                    return;
                };

                match arg {
                    "/" => {
                        self.cwd = None;
                    }
                    ".." => {
                        self.cwd = self.cwd.and_then(|cwd| world.parent_of(cwd));
                    }
                    name => {
                        // Path form (supports absolute/relative):
                        //   cd /7v1:root/8v1:child
                        //   cd 7v1:child/grandchild
                        if name.contains('/') {
                            match self.cd_path(world, name) {
                                Ok(new_cwd) => self.cwd = new_cwd,
                                Err(e) => println!("🐈 cd: {}", e),
                            }
                            return;
                        }

                        let candidates: Vec<ecs::ComponentId> = self.current_listing(world);

                        // 1) If it's a numeric index, treat it as an index into the last listing.
                        if let Ok(idx) = name.parse::<usize>() {
                            if let Some(cid) = candidates.get(idx).copied() {
                                self.cwd = Some(cid);
                            } else {
                                println!("🐈 cd: index out of range: {}", idx);
                            }
                            return;
                        }

                        // 2) If it parses as a UUID, match on GUID.
                        if let Ok(guid) = name.parse::<uuid::Uuid>() {
                            let mut found: Option<ecs::ComponentId> = None;
                            for cid in candidates.iter().copied() {
                                if let Some(node) = world.get_component_node(cid) {
                                    if node.guid == guid {
                                        found = Some(cid);
                                        break;
                                    }
                                }
                            }

                            // If not found in the current listing, allow a global jump-by-guid.
                            if found.is_none() {
                                found = world.component_id_by_guid(guid);
                            }

                            match found {
                                Some(cid) => self.cwd = Some(cid),
                                None => println!("🐈 cd: guid not found: {}", guid),
                            }
                            return;
                        }

                        // 3) Otherwise, treat it as a name.
                        let mut matches: Vec<ecs::ComponentId> = Vec::new();
                        for cid in candidates.iter().copied() {
                            if let Some(node) = world.get_component_node(cid) {
                                if node.name == name {
                                    matches.push(cid);
                                }
                            }
                        }

                        match matches.len() {
                            0 => println!("🐈 cd: not found: {}", name),
                            1 => self.cwd = Some(matches[0]),
                            _ => {
                                println!("🐈 cd: ambiguous name: {}", name);
                                println!("🐈 hint: use 'ls' then 'cd <index>' or 'cd <guid>'");
                            }
                        }
                    }
                }
            }
            _ => println!("🐈 unknown command: {}", verb),
        }
    }

    /// Execute all queued commands.
    pub fn exec_all<I>(&mut self, world: &ecs::World, commands: I)
    where
        I: IntoIterator<Item = String>,
    {
        for cmd in commands {
            self.exec(world, &cmd);
        }
    }
}
