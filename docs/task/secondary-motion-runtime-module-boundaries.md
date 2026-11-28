# Secondary motion runtime module boundaries

Proposed before extending the oversized secondary-motion system and MMS
component method registry. Separate chain scheduling and pause/resume state
from topology/binding and solver iteration; keep MMS method dispatch grouped
by component family. Preserve retained owner indexes and direct batched hot
paths, avoiding per-chain allocation, repeated lookups or pipeline creation.
Validate per-avatar and global pause interactions, pose continuity, topology
changes and removal, and compare frame timings with multiple active avatars.
