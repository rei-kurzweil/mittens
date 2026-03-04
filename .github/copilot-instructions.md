
# Copilot instructions

## Compatibility policy

- Do **not** add backward compatibility for JSON schema changes.
	- No legacy field-name aliases in `decode()`.
	- No accepting multiple shapes/types for the same field to support older saved data.
	- No keeping old component type names in `component_codec` as aliases.
- Do **not** keep old method/type names “for compatibility” after a rename.

