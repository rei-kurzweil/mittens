use super::*;

impl<I> RuntimeSpecBuilder<I> {
    pub fn with_standard_builtins(&mut self) -> &mut Self {
        for name in [
            "null",
            "range",
            "len",
            "byte_len",
            "query",
            "query_all",
            "world",
            "on",
            "on_global",
            "Math",
            "MusicNote",
        ] {
            self.pure_builtin(name, ValueSignature::any());
        }
        self.pure_builtin(
            "is_string",
            ValueSignature::new(vec![ValueType::Any], ValueType::Bool),
        );
        self
    }
}
