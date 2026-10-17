// Proposed component APIs, not executable engine registrations.
// This fixture tests existing MMS parsing/materialization of named operands.
// First-builder classification still needs implicit-default-construction support.
VectorArithmetic
    .subtract()
    .left(ValueSource.constant([1.0, 2.0, 3.0]))
    .right(
        VectorRotate
            .rotation(ValueSource.constant([0.0, 0.0, 0.0, 1.0]))
            .vector(ValueSource.constant([0.0, 0.12, 0.0]))
    )
