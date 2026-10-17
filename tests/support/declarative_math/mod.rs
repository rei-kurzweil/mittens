pub mod mms;
pub mod native;

#[derive(Clone, Copy, Debug)]
pub struct BatchOutput {
    pub checksum: f64,
    pub last: [f64; 4],
}

pub fn native_batch(
    backend: &mut impl native::NativeBackend,
    inputs: &[native::Input],
) -> BatchOutput {
    let mut output = BatchOutput {
        checksum: 0.0,
        last: [0.0; 4],
    };
    for input in inputs {
        let value = backend.evaluate(std::hint::black_box(*input));
        output.checksum += value.checksum();
        output.last = value.components();
    }
    output
}
