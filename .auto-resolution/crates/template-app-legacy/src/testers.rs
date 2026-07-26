// https://docs.rs/minijinja/latest/minijinja/tests/index.html

pub fn todo(v: impl Into<String>) -> impl minijinja::tests::Test<bool, (String,)> {
    let v = v.into();
    move |value: String| value == v
}
