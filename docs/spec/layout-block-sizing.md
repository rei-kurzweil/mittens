# Block Layout Sizing

## Rule

For `display: block`, if a node does not specify `Style.height`, its height is the smallest height that fits its content.

In engine terms, an unspecified height and `height: Auto` are the same contract for block layout:

- resolve height from intrinsic content size
- include child/content measurement plus padding/border semantics owned by the layout model
- do not stretch to consume remaining parent height just because the parent has a known height

## Implications

- block items stack vertically based on their own measured box heights
- remaining-space distribution is not part of block auto sizing
- any future fill/stretch behavior should be expressed by a different layout mode or an explicit sizing rule

## Non-rule

The following is not valid block-layout behavior:

- treating `display: block` items with unspecified height as equal-share fill items
- dividing leftover container height across block children by default

That behavior belongs to a flex-like layout contract, not block layout.

## Notes

- This rule is about block-axis sizing only
- width may still default to the normal block-layout width behavior for the containing block
- this spec describes intended behavior even where current implementation still differs