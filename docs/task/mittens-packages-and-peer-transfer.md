# Task: Mittens packages and peer asset transfer

## Status

Proposed, not implemented. Existing pieces: the confirmation UI
[asset_transfer_panel.mms](../../assets/components/ui/asset_transfer_panel.mms)
(mock data only, see its preview) and the saved
`NetworkSettings.asset_transfers.auto_accept()` setting, which nothing reads
yet. The [Android LAN workflow](../../android/docs/mms-lan-development.md) uses
the same manifest + content digest + cache model for developer project
transfer; share the hashing and cache code with it rather than building two.

## Outcome

What matters is the set of packages **in use** by a running server or client.
Each peer can state it as "I'm using these packages (by hash); which do you
lack?". So assets that peers share load through packages even locally: avatars
first, each as a versioned package, then props and emotes. Over the network,
peers exchange only the files the other side lacks, identified by content
hash, after the receiver confirms. A packages panel in the dock browses,
imports and uninstalls them.

## Package

A package is one `manifest.mms` plus zero or more `.mms` and `.glb` files.
There are two practical kinds:

- **Script only**: an emote, an emote pack, a behaviour.
- **Script + GLB**: an avatar, a prop, a vehicle.

Installed packages are uncompressed folders, so they load and hash in place.
Compression happens only in transit (and optionally in a `.zip` of the same
layout for sharing by hand).

### manifest.mms

Data only. It is read before any package script runs, so the receiver can see
what a package needs and asks for before trusting it: a single exported table
literal, evaluated with no host APIs, imports or callbacks. Anything else is
rejected.

```mms
export let package = {
    name = "Pilot"
    version = "1.2.0"
    entry = "pilot.mms"             // exports the factory / emote definitions
    size_class = "humanoid"         // placeholder while loading
    perf = { triangles = 48000 bones = 92 materials = 7 spring_chains = 6 }
    permissions = ["avatar_scope"]  // see below
}
```

File hashes are not written by hand. The packer and loader compute them; the
network manifest (below) carries them.

### Properties

All three apply to either kind of package:

- **Size class**: what to show while the package loads (humanoid
  placeholder, small, large). This replaces `asset_kind` in the transfer
  panel; "script" was never a size and becomes the package kind.
- **Performance class**: triangles, bones, materials, spring chains and
  texture memory. These are measured on import rather than trusted from the
  manifest, and the server can set limits.
- **Script permissions**: the APIs and event scopes the scripts use, for
  example only the owning avatar's subtree and emote events (`avatar_scope`),
  global event listeners, spawning, network channels, input. Check whether
  they can be derived from the scripts against the RuntimeSpec namespaces and
  checked against the declared list. The server allows or blocks each one.
  The confirm panel shows them. Auto-accept covers only packages within the
  user's limits.

Custom emote logic is the motivating case: a package's scripts may react
when its emote fires. That needs an emote event on the avatar scope (see
[emotes](emotes-and-emote-wheel.md)), which no doc defines yet. Reacting to
other players' emotes needs a wider permission.

## Installed packages and the in-use set

Exact folders matter less than the runtime state:

- **Installed**: packages Mittens knows about.
  - Built-in packages ship with the repo, under a tracked folder.
  - Imported and received packages live in an installed store under the user
    data directory (next to `user_config::directory()`), outside git, so
    copyrighted or private models never end up in a commit.
  - Each record has: name, version, package hash, source (built-in,
    imported from a path, or received from a peer) and install time.
- **In use**: the packages the current session has loaded. A scene, avatar
  picker or emote wheel asks for a package by name/version or hash. Loading
  one adds it to the in-use set, and that set is what a peer is told about on
  join and when it changes.

Received packages are installed like any other, marked with the peer they
came from. Files still referenced by the in-use set are never removed.

A package never refers to files outside its own folder: no `..`, no absolute
paths, no symlinks. That keeps local and transferred packages identical.

## Packages panel (dock)

- Lists installed packages: name, version, kind, size class, perf class,
  permissions, source, disk size and whether it's in use.
- **Import**: paste or type a path. A directory comes first; a `.zip` comes
  later with the zip slice. The importer validates `manifest.mms` and paths,
  hashes the files and copies them into the installed store, so later edits
  to the source folder don't silently change an installed version.
  Re-importing changed content installs a new version.
- **Uninstall**: imported and received packages only, never built-ins.
  Packages in use are rejected with a clear message.
- **Receive**: incoming offers still go through the transfer panel.

## Identity and negotiation

- **Identity**: every file's identity is its content hash; name and path are
  only for display and cache layout. A package's identity is the hash of its
  sorted (path, hash, size) list. A newer avatar under the same file name is
  a different package, and a GLB shared by two packages is sent once.
- **Hash**: BLAKE3 or SHA-256, not MD5. Peers are untrusted, and MD5
  collisions are cheap to make: one file could be approved and a different
  file with the same hash swapped in. No hash crate is in `Cargo.toml` yet.
  Cache hashes by (path, size, mtime) so unchanged local files are not
  re-hashed. Hash off the XR and render threads.
- **Exchange**:
  1. The owner sends the network manifest: package hash, `manifest.mms`
     contents, and the (path, hash, size) list.
  2. The receiver replies with the hashes it lacks.
  3. The receiver confirms: the panel lists only missing files, plus the
     permissions and perf class. Files already present show "Already have".
  4. Missing files are sent, and each is checked against its hash before
     being written to the cache.
- On join and whenever the in-use set changes, each peer sends its in-use
  package hashes. The other side answers with the ones it lacks, and each of
  those goes through the exchange above.
- Players refer to avatars and emotes by package hash plus the name inside
  the package, not by a path. This extends the avatar id in the
  [avatar selection spec](../spec/avatar-selection-and-swapping.md).

## Zip

- **Compression**: GLB compresses unevenly. Embedded PNG/JPEG/KTX2 textures,
  usually most of an avatar's bytes, barely shrink. Vertex, index and joint
  buffers shrink more, and meshopt/Draco-compressed GLBs barely at all. MMS
  text compresses very well. Deflate each entry and fall back to stored when
  that doesn't help. Measure on the bundled Mitten and VRoid models before
  choosing a default.
- **Transfer**: send individual files rather than the zip, so negotiation
  works per file. The sender can compress each file in transit. The zip is
  for distribution and backup.
- **Safety**: reject zip-slip paths, duplicate entries, symlinks and
  encrypted entries. Cap entry count, per-entry and total uncompressed size,
  and compression ratio, checking the actual bytes, not the sizes the zip
  header claims.

## Transfer panel changes

- Rows are grouped by package, with kind, size class, perf class and
  permissions shown.
- Each file row gains `hash` and an "Already have" state for files that will
  not be sent.
- `asset_kind` becomes the package's `size_class`.
- Per-package accept/decline replaces "Accept all" once the panel shows
  several packages.

## Out of scope

- Uploading to a central server or store.
- Editing packages in Mittens.
- Formats other than `.mms` and `.glb`, such as separate textures or audio.
  Add them as kinds when needed.

## Slices

1. Data-only `manifest.mms` reader, path checks, installed store and
   in-use set. Convert the built-in avatars into versioned packages and load
   them through it.
2. File and package hashing with the (path, size, mtime) hash cache. Share it
   with the Android LAN transfer.
3. Network manifest, missing-hash exchange and verified writes into the
   content cache, over a reliable channel.
4. Panel driven by real negotiation, with the new fields, honouring
   auto-accept within limits.
5. Packages panel in the dock: browse, import a directory by path,
   uninstall.
6. Measured perf class, and server limits.
7. Script permission derivation and server policy, plus the avatar-scope emote
   event.
8. Zip import and export with the safety checks.

## Validation

- Headless tests:
  - two peers with overlapping caches transfer only the missing hashes;
  - a renamed file with the same content is not sent;
  - a same-named file with new content is sent;
  - a corrupted or substituted file is rejected;
  - manifests with code, imports or escaping paths are rejected;
  - zip bombs and zip-slip entries are rejected;
  - the in-use set is announced on join and on change;
  - importing copies and hashes the files, and re-importing changed content
    installs a new version;
  - uninstalling a package that is in use or built in is rejected.
- Measure zip size and pack time on real avatars.
- Headset: a peer's avatar arrives, shows its placeholder, and swaps in on the
  Focus 3 without stalling frames.
