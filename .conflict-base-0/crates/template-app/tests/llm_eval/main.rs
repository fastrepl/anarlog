mod cases;
mod support;

use hypr_template_eval::{Arguments, run_case_suite, samples_from_env};

fn main() {
    let samples = samples_from_env();
    run_case_suite(Arguments::from_args(), cases::all(samples));
}
